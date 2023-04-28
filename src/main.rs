use std::{env, path::Path, process::Command, sync::Arc, thread, time::Duration};

use clap::Parser;

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
// - Add proper signal handling with signal_hook
//

#[derive(Parser, Debug)]
#[command(author, about, version)]
struct Args {
    #[arg(short, long, default_value_t = false)]
    verbose: bool,
    #[arg(short, long)]
    working_directory: Option<std::path::PathBuf>,
    #[arg(short, long, default_value_t = 5)]
    sleep_time: u32,
}

#[derive(Clone, Debug)]
struct DaemonConfig {
    // The amount of time to sleep between each iteration of the consumer loop.
    sleep_time: u32,
    // The working directory of the Magento 2 installation.
    magento_dir: String,
}

enum EnvironmentError {
    MagentoDirNotFound,
    MagentoBinNotFound,
    MagentoCronWorkerEnabled,
}

fn read_config(args: &Args) -> DaemonConfig {
    DaemonConfig {
        sleep_time: args.sleep_time,
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

fn run_consumer(config: &DaemonConfig, consumer: &String) {
    log::debug!("Running consumer: {}", consumer);
    Command::new("bin/magento")
        .current_dir(&config.magento_dir)
        .arg("queue:consumers:start")
        .arg(consumer)
        .output()
        .expect("failed to run bin/magento queue:consumers:start");
}

fn run_consumer_thread(config: Arc<DaemonConfig>, consumer: String) -> thread::JoinHandle<()> {
    thread::spawn(move || loop {
        run_consumer(&config, &consumer);
        thread::sleep(Duration::from_secs(config.sleep_time as u64));
    })
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
                EnvironmentError::MagentoDirNotFound => log::error!("error: Magento directory not found"),
                EnvironmentError::MagentoBinNotFound => log::error!("error: Magento bin not found"),
                EnvironmentError::MagentoCronWorkerEnabled => log::error!("error: Magento cron worker is enabled. Please see https://devdocs.magento.com/guides/v2.3/config-guide/mq/manage-message-queues.html#configuration to see how to disable the cron_run variable."),
            }
            std::process::exit(1);
        }
    }

    log::debug!("Fetching consumer list...");
    let consumers = read_consumer_list(&config);
    log::info!("Found {} consumers", consumers.len());

    let mut threads: Vec<thread::JoinHandle<()>> = Vec::new();
    for consumer in consumers {
        let config = Arc::new(config.clone());
        let thread = run_consumer_thread(config, consumer);
        threads.push(thread);
    }
    log::info!("Started {} threads", threads.len());

    for thread in threads {
        thread.join().unwrap();
    }
}
