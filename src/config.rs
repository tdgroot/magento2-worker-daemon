use std::{env, path::Path, process::Command};

use input::Args as InputArgs;

use crate::input;

#[derive(Clone, Debug)]
pub struct DaemonConfig {
    pub magento_dir: String,
    pub rabbitmq_configured: bool,
}

impl DaemonConfig {
    pub fn new(args: &InputArgs) -> DaemonConfig {
        let magento_dir = match args.working_directory {
            Some(ref path) => path.to_str().unwrap().to_string(),
            None => env::current_dir().unwrap().to_str().unwrap().to_string(),
        };
        let rabbitmq_configured = magento_has_rabbitmq_configured(&magento_dir);

        DaemonConfig {
            magento_dir,
            rabbitmq_configured,
        }
    }
}

pub enum EnvironmentError {
    MagentoDirNotFound,
    MagentoBinNotFound,
    MagentoCronWorkerEnabled,
}

fn magento_has_rabbitmq_configured(magento_dir: &String) -> bool {
    const RABBITMQ_CONFIGURED_QUERY: &str = r#"
    $config = include 'app/etc/env.php';
    $v = isset($config['queue']['amqp']);
    var_dump($v);
    "#;

    let output = Command::new("php")
        .current_dir(magento_dir)
        .args(&["-r", RABBITMQ_CONFIGURED_QUERY])
        .output()
        .expect("Failed to query rabbitmq configuration");
    output.stdout.eq(b"bool(true)\n")
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

pub fn validate_config(config: &DaemonConfig) -> Result<(), EnvironmentError> {
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
