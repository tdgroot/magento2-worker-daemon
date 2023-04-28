mod util;

use std::{
    env,
    path::Path,
    process::Command,
    sync::{atomic::AtomicBool, Arc},
    thread,
    time::Duration,
};

use clap::Parser;
use signal_hook::consts::TERM_SIGNALS;

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
// TODO:
// - Add unit tests
//

#[derive(Parser, Debug)]
#[command(author, about, version)]
struct Args {
    #[arg(short, long, default_value_t = false)]
    verbose: bool,
    #[arg(short, long)]
    working_directory: Option<std::path::PathBuf>,
}

#[derive(Clone, Debug)]
struct DaemonConfig {
    // The working directory of the Magento 2 installation.
    magento_dir: String,
}

#[derive(Debug)]
struct WorkerProcess {
    // The consumer name
    consumer: String,
    // The process handle
    process: std::process::Child,
}

impl WorkerProcess {
    fn terminate(&mut self) {
        log::debug!("Terminating consumer: {}", self.consumer);
        util::terminate_process_child(&self.process).unwrap();
        if self.process.try_wait().unwrap().is_none() {
            self.process.kill().unwrap();
        }
    }

    fn is_running(&mut self) -> bool {
        match self.process.try_wait() {
            Ok(Some(_)) => false,
            Ok(None) => true,
            Err(_) => false,
        }
    }

    fn restart(&mut self, config: &DaemonConfig) {
        if self.is_running() {
            self.terminate();
        }
        self.process = run_consumer(config, &self.consumer).process;
    }
}

enum EnvironmentError {
    MagentoDirNotFound,
    MagentoBinNotFound,
    MagentoCronWorkerEnabled,
}

fn read_config(args: &Args) -> DaemonConfig {
    DaemonConfig {
        magento_dir: match args.working_directory {
            Some(ref path) => path.to_str().unwrap().to_string(),
            None => env::current_dir().unwrap().to_str().unwrap().to_string(),
        },
    }
}

fn magento_cron_worker_is_disabled(config: &DaemonConfig) -> bool {
    const CRON_RUN_QUERY: &str = r#"
    $config = include 'app/etc/env.php';
    $v = $config['cron_consumers_runner']['cron_run'] ?? true;
    var_dump($v);
    "#;

    let output = Command::new("php")
        .current_dir(&config.magento_dir)
        .args(&["-r", CRON_RUN_QUERY])
        .output()
        .expect("Failed to query cron worker setting");
    output.stdout.eq(b"bool(false)\n")
}

fn validate_config(config: &DaemonConfig) -> Result<(), EnvironmentError> {
    // Check if magento dir exists
    let magento_dir_path = Path::new(&config.magento_dir);
    if !magento_dir_path.exists() {
        return Err(EnvironmentError::MagentoDirNotFound);
    }

    // Check if bin/magento exists
    if !magento_dir_path.join("bin/magento").exists() {
        return Err(EnvironmentError::MagentoBinNotFound);
    }

    // Check if cron worker spawner is disabled
    if !magento_cron_worker_is_disabled(&config) {
        return Err(EnvironmentError::MagentoCronWorkerEnabled);
    }

    Ok(())
}

fn read_consumer_list(config: &DaemonConfig) -> Vec<String> {
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
        .collect()
}

fn run_consumer(config: &DaemonConfig, consumer: &String) -> WorkerProcess {
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

fn configure_logging(args: &Args) {
    if args.verbose {
        simple_logger::init_with_level(log::Level::Debug).unwrap();
    } else {
        simple_logger::init_with_level(log::Level::Info).unwrap();
    }
}

fn main() {
    let args = Args::parse();
    configure_logging(&args);

    let config = read_config(&args);
    match validate_config(&config) {
        Ok(_) => {}
        Err(err) => {
            match err {
                EnvironmentError::MagentoDirNotFound => log::error!("Magento directory not found"),
                EnvironmentError::MagentoBinNotFound => log::error!("Magento bin not found"),
                EnvironmentError::MagentoCronWorkerEnabled => log::error!("Magento cron worker is enabled. Please see https://devdocs.magento.com/guides/v2.3/config-guide/mq/manage-message-queues.html#configuration to see how to disable the cron_run variable."),
            }
            std::process::exit(1);
        }
    }

    log::debug!("Fetching consumer list...");
    let consumers = read_consumer_list(&config);
    log::info!("Found {} consumers", consumers.len());

    let mut processes: Vec<WorkerProcess> = consumers
        .iter()
        .map(|consumer| run_consumer(&config, &consumer))
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
