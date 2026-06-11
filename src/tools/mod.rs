pub(crate) mod capabilities;
mod claude;
mod codex;
mod registry;
mod runner;

pub(crate) use capabilities::{mcp::McpServer, plugin::Plugin};
pub(crate) use registry::{all_tools, tool_for, tool_sort_key};
pub(crate) use runner::{CommandRunner, ProcessCommandRunner};

use crate::config::Target;

pub(crate) trait CodingAgentTool: Sync {
    fn target(&self) -> Target;
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn command(&self) -> &'static str;

    fn env(&self) -> &dyn capabilities::env::EnvCapability;
    fn shell(&self) -> Option<&dyn capabilities::shell::ShellCapability>;
    fn mcp(&self) -> Option<&dyn capabilities::mcp::McpCapability>;
    fn plugin(&self) -> Option<&dyn capabilities::plugin::PluginCapability>;
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum ToolError {
    CommandFailed {
        command: String,
        source: String,
    },
    NonZeroExit {
        command: String,
        stdout: String,
        stderr: String,
    },
    Timeout {
        command: String,
    },
    ConfigFailed {
        path: String,
        source: String,
    },
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolError::CommandFailed { command, source } => {
                write!(f, "Failed to run {command}: {source}")
            }
            ToolError::NonZeroExit {
                command,
                stdout,
                stderr,
            } => write!(f, "{command} failed: {stdout}{stderr}"),
            ToolError::Timeout { command } => write!(f, "{command} timed out"),
            ToolError::ConfigFailed { path, source } => {
                write!(f, "Failed to update {path}: {source}")
            }
        }
    }
}

impl std::error::Error for ToolError {}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn registry_covers_every_target_once() {
        let targets: Vec<Target> = all_tools().iter().map(|tool| tool.target()).collect();
        let unique: HashSet<Target> = targets.iter().copied().collect();

        assert_eq!(targets.len(), unique.len());
        assert!(
            Target::all()
                .into_iter()
                .all(|target| unique.contains(&target))
        );
    }

    #[test]
    fn registry_returns_codex_tool() {
        let tool = tool_for(Target::Codex);

        assert_eq!(tool.id(), "codex");
        assert_eq!(tool.command(), "codex");
        assert_eq!(tool.display_name(), "Codex");
    }

    #[test]
    fn registry_returns_claude_tool() {
        let tool = tool_for(Target::Claude);

        assert_eq!(tool.id(), "claude");
        assert_eq!(tool.command(), "claude");
        assert_eq!(tool.display_name(), "Claude Code");
    }

    #[test]
    fn registry_sorts_codex_before_claude() {
        assert!(tool_sort_key(Target::Codex) < tool_sort_key(Target::Claude));
    }
}
