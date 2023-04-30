mod config;
mod input;
mod util;
mod worker;

use std::{
    sync::{atomic::AtomicBool, Arc},
    thread,
    time::Duration,
};

use signal_hook::consts::TERM_SIGNALS;

use input::Args as InputArgs;

use crate::worker::WorkerProcess;

//
// Magento 2 Worker Daemon
//
// This is a daemon that runs Magento 2 queue consumers in the background.
// It is designed to be run as a systemd/supervisor service.
//
// The daemon works as follows:
// 1. It reads the available queue consumers from the Magento 2 configuration.
//    This is done by running bin/magento queue:consumers:list
// 2. It then spawns a thread for each consumer, which runs the consumer in a loop.
// 3. The consumer is run in a loop, and sleeps for a configurable amount of time
//    between each iteration.
//

fn configure_logging(args: &InputArgs) {
    if args.verbose {
        simple_logger::init_with_level(log::Level::Debug).unwrap();
    } else {
        simple_logger::init_with_level(log::Level::Info).unwrap();
    }
}

fn main() {
    let args = input::parse_args();
    configure_logging(&args);

    let config = config::read_config(&args);
    match config::validate_config(&config) {
        Ok(_) => {}
        Err(err) => {
            use config::EnvironmentError::*;
            match err {
                MagentoDirNotFound => log::error!("Magento directory not found"),
                MagentoBinNotFound => log::error!("Magento bin not found"),
                MagentoCronWorkerEnabled => log::error!("Magento cron worker is enabled. Please see https://devdocs.magento.com/guides/v2.3/config-guide/mq/manage-message-queues.html#configuration to see how to disable the cron_run variable."),
            }
            std::process::exit(1);
        }
    }

    log::debug!("Fetching consumer list...");
    let consumers = worker::read_consumer_list(&config);
    log::info!("Found {} consumers", consumers.len());

    let mut processes: Vec<WorkerProcess> = consumers
        .iter()
        .map(|consumer| worker::run_worker(&config, &consumer))
        .collect();
    log::info!("Started {} consumers", processes.len());

    let term = Arc::new(AtomicBool::new(false));
    for sig in TERM_SIGNALS {
        signal_hook::flag::register(*sig, Arc::clone(&term)).unwrap();
    }

    while !term.load(std::sync::atomic::Ordering::Relaxed) {
        // If any of the processes have exited, restart them
        for process in &mut processes {
            if !process.is_running() {
                process.restart(&config);
            }
        }
        thread::sleep(Duration::from_secs(2));
    }

    log::info!("Stopping {} consumers", processes.len());
    for mut process in processes {
        process.terminate();
    }
}
