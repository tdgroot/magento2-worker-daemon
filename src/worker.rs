use std::process::Command;

use crate::{
    config::{DaemonConfig, DaemonContext},
    util,
};

const RABBITMQ_CONSUMER_NAMES: [&str; 1] = ["async.operations.all"];

#[derive(Debug)]
pub struct WorkerProcess {
    // The consumer name
    consumer: String,
    // The process handles
    processes: Vec<std::process::Child>,
}

impl WorkerProcess {
    pub fn terminate(&mut self) {
        log::debug!("Terminating consumer: {}", self.consumer);
        for p in self.processes.iter_mut() {
            util::terminate_process_child(&p).unwrap();
            if p.is_running() {
                log::debug!("Force killing consumer: {}", self.consumer);
                p.kill().unwrap();
            }
        }
    }

    pub fn ensure_running(&mut self, context: &DaemonContext) {
        if !self.processes.is_running() {
            self.restart(context);
        }
    }

    pub fn restart(&mut self, context: &DaemonContext) {
        self.terminate();
        self.processes = run_worker(&context, &self.consumer).processes;
    }
}

trait WorkerChildProcess {
    fn is_running(&mut self) -> bool;
}

impl WorkerChildProcess for std::process::Child {
    fn is_running(&mut self) -> bool {
        match self.try_wait() {
            Ok(Some(status)) => {
                log::debug!("Process exited with status {:?}", status);
                return false;
            }
            Ok(None) => true,
            Err(err) => {
                log::debug!("Process has error {:?}", err);
                return false;
            }
        }
    }
}

impl WorkerChildProcess for Vec<std::process::Child> {
    fn is_running(&mut self) -> bool {
        self.iter_mut().all(|p| p.is_running())
    }
}

pub fn read_consumer_list(config: &DaemonConfig) -> Vec<String> {
    let output = Command::new("bin/magento")
        .current_dir(&config.magento_dir)
        .arg("queue:consumers:list")
        .output()
        .expect("Failed to run bin/magento queue:consumers:list");

    // Split output by newline and convert from u8 sequences to String
    output
        .stdout
        .split(|&x| x == b'\n')
        .map(|x| String::from_utf8(x.to_vec()).unwrap())
        .filter(|x| !x.is_empty())
        .filter(|x| {
            // Filter out rabbitmq consumers when rabbitmq is not configured
            if !config.rabbitmq_configured {
                !RABBITMQ_CONSUMER_NAMES.contains(&x.as_str())
            } else {
                true
            }
        })
        .collect()
}

pub fn run_worker(context: &DaemonContext, consumer: &String) -> WorkerProcess {
    log::debug!("Running consumer: {}", consumer);

    let mut number_of_processes = 1;
    if let Some(processes) = context.consumer_config.multiple_processes.get(consumer) {
        number_of_processes = *processes;
    }

    let mut processes = Vec::<std::process::Child>::new();

    for i in 0..number_of_processes {
        let mut command = Command::new("bin/magento");
        let command = command
            .current_dir(&context.daemon_config.magento_dir)
            .arg("queue:consumers:start")
            .arg(consumer)
            .arg("--max-messages")
            .arg(context.consumer_config.max_messages.to_string());

        // We could disable the --multi-process or --single-thread options with a --no-strict-mode flag,
        // but not sure if users need that, so this is the default for now.
        if number_of_processes > 1 {
            command.arg("--multi-process");
            command.arg(i.to_string());
        } else {
            command.arg("--single-thread");
        }

        let process = command
            .spawn()
            .expect("Failed to run bin/magento queue:consumers:start");

        processes.push(process);
    }

    WorkerProcess {
        consumer: consumer.clone(),
        processes,
    }
}
