use std::process::{Command, ExitStatus};

pub fn terminate_process_child(process: &std::process::Child) -> std::io::Result<ExitStatus> {
    Command::new("kill")
        .arg("-SIGTERM")
        .arg(process.id().to_string())
        .spawn()
        .expect("failed to kill process")
        .wait()
}
