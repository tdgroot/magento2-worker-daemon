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

    let context = config::DaemonContext::new(&args).unwrap_or_else(|e| {
        log::error!("{}", e.message);
        std::process::exit(1);
    });

    log::debug!("Fetching consumer list...");
    let consumers = worker::read_consumer_list(&context.daemon_config);
    let consumers = consumers
        .iter()
        .filter(|x| {
            context.consumer_config.consumers.is_empty()
                || context.consumer_config.consumers.contains(x)
        })
        .collect::<Vec<_>>();
    log::info!("Found {} applicable consumers", consumers.len());

    let mut processes: Vec<WorkerProcess> = consumers
        .iter()
        .map(|consumer| worker::run_worker(&context, consumer))
        .collect();
    log::info!("Started {} consumers", processes.len());

    let term = Arc::new(AtomicBool::new(false));
    for sig in TERM_SIGNALS {
        signal_hook::flag::register(*sig, Arc::clone(&term)).unwrap();
    }

    while !term.load(std::sync::atomic::Ordering::Relaxed) {
        // If any of the processes have exited, restart them
        for process in &mut processes {
            process.ensure_running(&context);
        }
        thread::sleep(Duration::from_secs(2));
    }

    log::info!("Stopping {} consumers", processes.len());
    for mut process in processes {
        process.terminate();
    }
}
