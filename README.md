# Magento 2 Worker Daemon

Daemon for running Magento 2 queue consumers, designed to be run as a systemd/supervisor service.

This daemon acts as a drop-in replacement for the Magento 2 cron consumers runner, compatible with the Magento 2 [cron consumers runner configuration](https://experienceleague.adobe.com/docs/commerce-operations/configuration-guide/message-queues/manage-message-queues.html#configuration).

## Features

- Detects and runs all eligible Magento 2 queue consumers
  - RabbitMQ specific consumers are not run when RabbitMQ is not configured in Magento.
  - Compatible with the `cron_consumers_runner.consumers` setting to only run specified consumers.
  - Regards all settings in the `cron_consumers_runner` [environment configuration](https://experienceleague.adobe.com/docs/commerce-operations/configuration-guide/message-queues/manage-message-queues.html#configuration).
- Restarts consumers if they fail/stop
- Supports running consumers in a different working directory
- Validates Magento 2 installation before starting consumers

## Installation

The program is not available yet on any package repository, so for now you can to download it from the latest GitHub release.

```bash
wget --quiet https://github.com/tdgroot/magento2-worker-daemon/releases/latest/download/magento2-worker-daemon -O magento2-worker-daemon
chmod +x magento2-worker-daemon
./magento2-worker-daemon
```

## Usage

```console
$ magento2-worker-daemon
2023-04-28T13:36:12.788Z INFO  [magento2_worker_daemon] Found 19 consumers
2023-04-28T13:36:12.793Z INFO  [magento2_worker_daemon] Started 19 consumers
```

### Command line options

```console
$ magento2-worker-daemon --help
Usage: magento2-worker-daemon [OPTIONS]

Options:
  -v, --verbose                                Enable verbose logging
  -w, --working-directory <WORKING_DIRECTORY>  Magento 2 working directory
  -h, --help                                   Print help
  -V, --version                                Print version
```

## Configuration

First make sure you disable the `cron_consumers_runner.cron_run` setting in the `app/etc/env.php`:

```php
return [
    ...
    'cron_consumers_runner' => [
        'cron_run' => false
    ],
    ...
];
```

Also make sure you have the correct `php` binary in the `PATH` environment variable where you're going to run this.
So if you have PHP installed in a directory that is not in the default `PATH`, make sure you set the proper environment configuration for systemd/supervisor.

### Systemd

```ini
[Unit]
Description=Magento 2 Worker Daemon
After=network.target

[Service]
WorkingDirectory=/path/to/magento2
ExecStart=/usr/bin/magento2-worker-daemon
Restart=always

[Install]
WantedBy=multi-user.target
```

### Supervisor

```ini
[program:magento2-worker-daemon]
directory=/path/to/magento2
command=/usr/bin/magento2-worker-daemon
autostart=true
autorestart=true
stopsignal=INT
stopasgroup=true
```

