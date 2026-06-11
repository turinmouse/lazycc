use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::tools::ToolError;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct CommandOutput {
    pub(crate) success: bool,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

pub(crate) trait CommandRunner: Sync {
    fn run(&self, command: &str, args: &[&str]) -> Result<CommandOutput, ToolError>;
    fn run_with_timeout(
        &self,
        command: &str,
        args: &[&str],
        timeout: Duration,
    ) -> Result<CommandOutput, ToolError> {
        let _ = timeout;
        self.run(command, args)
    }
}

pub(crate) struct ProcessCommandRunner;

const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);
const COMMAND_POLL_INTERVAL: Duration = Duration::from_millis(25);

impl CommandRunner for ProcessCommandRunner {
    fn run(&self, command: &str, args: &[&str]) -> Result<CommandOutput, ToolError> {
        self.run_with_timeout(command, args, COMMAND_TIMEOUT)
    }

    fn run_with_timeout(
        &self,
        command: &str,
        args: &[&str],
        timeout: Duration,
    ) -> Result<CommandOutput, ToolError> {
        let command_line = format!("{} {}", command, args.join(" "));
        let mut child = Command::new(command)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| ToolError::CommandFailed {
                command: command_line.clone(),
                source: error.to_string(),
            })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ToolError::CommandFailed {
                command: command_line.clone(),
                source: "failed to capture stdout".to_string(),
            })?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| ToolError::CommandFailed {
                command: command_line.clone(),
                source: "failed to capture stderr".to_string(),
            })?;
        let stdout_reader = thread::spawn(move || {
            let mut reader = stdout;
            let mut output = Vec::new();
            let _ = reader.read_to_end(&mut output);
            String::from_utf8_lossy(&output).to_string()
        });
        let stderr_reader = thread::spawn(move || {
            let mut reader = stderr;
            let mut output = Vec::new();
            let _ = reader.read_to_end(&mut output);
            String::from_utf8_lossy(&output).to_string()
        });

        let started_at = Instant::now();
        let status = loop {
            match child.try_wait().map_err(|error| ToolError::CommandFailed {
                command: command_line.clone(),
                source: error.to_string(),
            })? {
                Some(status) => break status,
                None if started_at.elapsed() >= timeout => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = stdout_reader.join();
                    let _ = stderr_reader.join();
                    return Err(ToolError::Timeout {
                        command: command_line,
                    });
                }
                None => thread::sleep(COMMAND_POLL_INTERVAL),
            }
        };
        let stdout = stdout_reader.join().map_err(|_| ToolError::CommandFailed {
            command: command_line.clone(),
            source: "failed to read stdout".to_string(),
        })?;
        let stderr = stderr_reader.join().map_err(|_| ToolError::CommandFailed {
            command: command_line.clone(),
            source: "failed to read stderr".to_string(),
        })?;

        Ok(CommandOutput {
            success: status.success(),
            stdout,
            stderr,
        })
    }
}
