# Magento 2 Worker Daemon

Daemon for running Magento 2 queue consumers, designed to be run as a systemd/supervisor service.

## Features

- Detects and runs all Magento 2 queue consumers
- Restarts consumers if they fail/stop
- Supports running consumers in a different working directory
- Validates Magento 2 installation before starting consumers

## Installation

```bash
git clone https://github.com/tdgroot/magento2-worker-daemon.git
cargo run -- --working-directory /path/to/magento2
```

## Usage

```console
$ magento2-worker-daemon
2023-04-28T13:36:12.788Z INFO  [magento2_worker_daemon] Found 19 consumers
2023-04-28T13:36:12.793Z INFO  [magento2_worker_daemon] Started 19 t
```

### Command line options

```console
$ magento2-worker-daemon --help
Usage: magento2-worker-daemon [OPTIONS]

Options:
  -v, --verbose                                
  -w, --working-directory <WORKING_DIRECTORY>  
  -s, --sleep-time <SLEEP_TIME>                [default: 5]
  -h, --help                                   Print help
  -V, --version                                Print version
```

## Configuration

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

## Work in progress

This project is still a work in progress, and is not yet ready for production use.

Things that still need to be done:
- Add unit tests
- Add proper signal handling (SIGTERM, SIGINT, SIGQUIT)
  - Currently, the daemon will only stop child processes when it receives a SIGINT
