use std::{collections::HashMap, env, path::Path, process::Command};

use input::Args as InputArgs;

use serde::Deserialize;

use crate::input;

#[derive(Clone, Debug)]
pub struct DaemonConfig {
    pub magento_dir: String,
    pub rabbitmq_configured: bool,
}

#[derive(Debug, Deserialize)]
pub struct MagentoConsumerConfig {
    #[serde(default = "default_cron_run")]
    cron_run: bool,
    #[serde(default = "default_max_messages")]
    pub max_messages: u32,
    #[serde(default)]
    pub consumers: Vec<String>,
    #[serde(default)]
    pub multiple_processes: HashMap<String, i32>,
}

#[derive(Debug)]
pub struct DaemonContext {
    pub daemon_config: DaemonConfig,
    pub consumer_config: MagentoConsumerConfig,
}

impl DaemonConfig {
    pub fn new(args: &InputArgs) -> Result<Self, EnvironmentError> {
        let magento_dir = match args.working_directory {
            Some(ref path) => path.to_str().unwrap().to_string(),
            None => env::current_dir().unwrap().to_str().unwrap().to_string(),
        };
        let rabbitmq_configured = magento_has_rabbitmq_configured(&magento_dir);

        let result = Self {
            magento_dir,
            rabbitmq_configured,
        };
        result.validate()?;
        Ok(result)
    }

    pub fn validate(&self) -> Result<(), EnvironmentError> {
        // Check if magento dir exists
        let magento_dir_path = Path::new(&self.magento_dir);
        if !magento_dir_path.exists() {
            return Err(EnvironmentError {
                message: "Magento directory not found".to_owned(),
            });
        }

        // Check if bin/magento exists
        if !magento_dir_path.join("bin/magento").exists() {
            return Err(EnvironmentError {
                message: "Magento bin not found".to_owned(),
            });
        }

        Ok(())
    }
}

impl MagentoConsumerConfig {
    pub fn new(config: &DaemonConfig) -> Result<Self, EnvironmentError> {
        const CRON_RUN_QUERY: &str = r#"
        $config = include 'app/etc/env.php';
        $v = $config['cron_consumers_runner'] ?? [];
        if (isset($v['multiple_processes']) && empty($v['multiple_processes'])) {
            unset($v['multiple_processes']);
        }
        if (empty($v)) {
            // If the array is empty, we want to make sure we return a json dict instead of array.
            $v = new stdClass();
        }
        echo json_encode($v);
        "#;
        let output = Command::new("php")
            .current_dir(&config.magento_dir)
            .args(&["-r", CRON_RUN_QUERY])
            .output()
            .expect("Can query Magento consumer configuration");

        let consumer_config: Self = serde_json::from_slice(&output.stdout).unwrap();
        consumer_config.validate()?;
        Ok(consumer_config)
    }

    pub fn validate(&self) -> Result<(), EnvironmentError> {
        if self.cron_run {
            return Err(EnvironmentError{
                message: "Magento cron worker is enabled. Please see https://experienceleague.adobe.com/docs/commerce-operations/configuration-guide/message-queues/manage-message-queues.html#configuration to see how to disable the cron_run variable.".to_owned()
            });
        }
        if self.multiple_processes.values().any(|x| *x < 0) {
            return Err(EnvironmentError {
                message: "Magento consumer multiple_processes values must be greater than zero"
                    .to_owned(),
            });
        }
        Ok(())
    }
}

impl DaemonContext {
    pub fn new(args: &InputArgs) -> Result<Self, EnvironmentError> {
        let config = DaemonConfig::new(args)?;
        let consumer_config = MagentoConsumerConfig::new(&config)?;
        Ok(Self {
            daemon_config: config,
            consumer_config,
        })
    }
}

pub struct EnvironmentError {
    pub message: String,
}

fn default_cron_run() -> bool {
    true
}

fn default_max_messages() -> u32 {
    10000
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
