use std::process::Command;

use crate::{config::DaemonConfig, util};

const RABBITMQ_CONSUMER_NAMES: [&str; 1] = ["async.operations.all"];

#[derive(Debug)]
pub struct WorkerProcess {
    // The consumer name
    consumer: String,
    // The process handle
    process: std::process::Child,
}

impl WorkerProcess {
    pub fn terminate(&mut self) {
        log::debug!("Terminating consumer: {}", self.consumer);
        util::terminate_process_child(&self.process).unwrap();
        if self.process.try_wait().unwrap().is_none() {
            self.process.kill().unwrap();
        }
    }

    pub fn is_running(&mut self) -> bool {
        match self.process.try_wait() {
            Ok(Some(_)) => false,
            Ok(None) => true,
            Err(_) => false,
        }
    }

    pub fn restart(&mut self, config: &DaemonConfig) {
        if self.is_running() {
            self.terminate();
        }
        self.process = run_worker(config, &self.consumer).process;
    }
}

pub fn read_consumer_list(config: &DaemonConfig) -> Vec<String> {
    // Read consumer list by running bin/magento queue:consumers:list
    let output = Command::new("bin/magento")
        .current_dir(&config.magento_dir)
        .arg("queue:consumers:list")
        .output()
        .expect("failed to run bin/magento queue:consumers:list");

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

pub fn run_worker(config: &DaemonConfig, consumer: &String) -> WorkerProcess {
    log::debug!("Running consumer: {}", consumer);
    let process = Command::new("bin/magento")
        .current_dir(&config.magento_dir)
        .arg("queue:consumers:start")
        .arg(consumer)
        .spawn()
        .expect("failed to run bin/magento queue:consumers:start");
    WorkerProcess {
        consumer: consumer.clone(),
        process,
    }
}
