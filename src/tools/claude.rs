use crate::config::Target;
use crate::tools::capabilities::env::EnvCapability;
use crate::tools::capabilities::mcp::{
    McpCapability, McpServer, list_servers_with_runner, remove_server_with_runner,
};
use crate::tools::capabilities::plugin::{
    PluginCapability, install_claude_plugin_with_runner, list_claude_available_plugins_with_runner,
    list_claude_plugins, remove_claude_plugin_with_runner, set_claude_plugin_enabled,
};
use crate::tools::{CodingAgentTool, Plugin, ProcessCommandRunner, ToolError};

pub(crate) static CLAUDE_TOOL: ClaudeTool = ClaudeTool;

pub(crate) struct ClaudeTool;

impl CodingAgentTool for ClaudeTool {
    fn target(&self) -> Target {
        Target::Claude
    }

    fn id(&self) -> &'static str {
        "claude"
    }

    fn display_name(&self) -> &'static str {
        "Claude Code"
    }

    fn command(&self) -> &'static str {
        "claude"
    }

    fn env(&self) -> &dyn EnvCapability {
        self
    }

    fn shell(&self) -> Option<&dyn crate::tools::capabilities::shell::ShellCapability> {
        None
    }

    fn mcp(&self) -> Option<&dyn McpCapability> {
        Some(self)
    }

    fn plugin(&self) -> Option<&dyn PluginCapability> {
        Some(self)
    }
}

impl EnvCapability for ClaudeTool {
    fn base_url_env(&self) -> &'static str {
        "ANTHROPIC_BASE_URL"
    }

    fn api_key_env(&self) -> &'static str {
        "ANTHROPIC_AUTH_TOKEN"
    }

    fn model_env(&self) -> Option<&'static str> {
        Some("ANTHROPIC_MODEL")
    }

    fn all_envs(&self) -> &'static [&'static str] {
        &[
            "ANTHROPIC_BASE_URL",
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_MODEL",
        ]
    }
}

impl McpCapability for ClaudeTool {
    fn list_servers(&self) -> Result<Vec<McpServer>, ToolError> {
        list_servers_with_runner(self.target(), self.command(), &ProcessCommandRunner)
    }

    fn remove_server(&self, name: &str) -> Result<(), ToolError> {
        remove_server_with_runner(self.command(), name, &ProcessCommandRunner)
    }
}

impl PluginCapability for ClaudeTool {
    fn list_plugins(&self) -> Result<Vec<Plugin>, ToolError> {
        list_claude_plugins()
    }

    fn list_available_plugins(&self) -> Result<Vec<Plugin>, ToolError> {
        list_claude_available_plugins_with_runner(&ProcessCommandRunner)
    }

    fn install_plugin(&self, selector: &str) -> Result<(), ToolError> {
        install_claude_plugin_with_runner(selector, &ProcessCommandRunner)
    }

    fn set_plugin_enabled(&self, name: &str, enabled: bool) -> Result<(), ToolError> {
        set_claude_plugin_enabled(name, enabled)
    }

    fn remove_plugin(&self, name: &str) -> Result<(), ToolError> {
        remove_claude_plugin_with_runner(name, &ProcessCommandRunner)
    }
}
