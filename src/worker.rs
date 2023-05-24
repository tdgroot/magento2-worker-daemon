use std::{process::Command, time::Duration};

use crate::{
    config::{DaemonConfig, DaemonContext},
    util::terminate_process_child,
};

const RABBITMQ_CONSUMER_NAMES: [&str; 1] = ["async.operations.all"];
const PROCESS_GRACEFUL_KILL_PERIOD: Duration = Duration::from_millis(500);
const PROCESS_GRACEFUL_POLL_RESOLUTION: Duration = Duration::from_millis(20);

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
            p.try_stop_gracefully(PROCESS_GRACEFUL_KILL_PERIOD);
        }
    }

    pub fn ensure_running(&mut self, context: &DaemonContext) {
        let is_running = self.processes.iter_mut().all(|p| p.is_running());
        if !is_running {
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
    fn try_stop_gracefully(&mut self, grace_period: Duration);
}

impl WorkerChildProcess for std::process::Child {
    fn is_running(&mut self) -> bool {
        match self.try_wait() {
            Ok(Some(status)) => {
                match status.code() {
                    Some(code) => log::debug!("Process {} exited with status {}", self.id(), code),
                    None => log::debug!("Process {} was terminated", self.id()),
                }
                return false;
            }
            Ok(None) => true,
            Err(err) => {
                log::debug!("Process has error {:?}", err);
                return false;
            }
        }
    }

    fn try_stop_gracefully(&mut self, grace_period: Duration) {
        if !self.is_running() {
            return;
        }

        let terminate_result = terminate_process_child(self);
        if terminate_result.is_err() {
            log::error!("Failed to SIGTERM process");
        }

        let mut waiting_time = 0;
        while self.is_running() {
            if waiting_time >= grace_period.as_millis() {
                self.kill().unwrap();
                log::debug!("Force killing process");
                break;
            }
            std::thread::sleep(PROCESS_GRACEFUL_POLL_RESOLUTION);
            waiting_time += PROCESS_GRACEFUL_POLL_RESOLUTION.as_millis();
        }

        // After it's killed, we need to call wait for the process to be removed from the process
        // table. For more information, see NOTES in man waitpid(2).
        self.wait().expect("Failed to wait for process to exit");
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
