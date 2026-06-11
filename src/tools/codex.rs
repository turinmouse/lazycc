use crate::config::Target;
use crate::tools::capabilities::env::EnvCapability;
use crate::tools::capabilities::mcp::{
    McpCapability, McpServer, list_servers_with_runner, remove_server_with_runner,
};
use crate::tools::capabilities::plugin::{
    PluginCapability, install_codex_plugin_with_runner, list_codex_available_plugins_with_runner,
    list_codex_plugins, remove_codex_plugin_with_runner, set_codex_plugin_enabled,
};
use crate::tools::capabilities::shell::ShellCapability;
use crate::tools::{CodingAgentTool, Plugin, ProcessCommandRunner, ToolError};

pub(crate) static CODEX_TOOL: CodexTool = CodexTool;

pub(crate) struct CodexTool;

impl CodingAgentTool for CodexTool {
    fn target(&self) -> Target {
        Target::Codex
    }

    fn id(&self) -> &'static str {
        "codex"
    }

    fn display_name(&self) -> &'static str {
        "Codex"
    }

    fn command(&self) -> &'static str {
        "codex"
    }

    fn env(&self) -> &dyn EnvCapability {
        self
    }

    fn shell(&self) -> Option<&dyn ShellCapability> {
        Some(self)
    }

    fn mcp(&self) -> Option<&dyn McpCapability> {
        Some(self)
    }

    fn plugin(&self) -> Option<&dyn PluginCapability> {
        Some(self)
    }
}

impl EnvCapability for CodexTool {
    fn base_url_env(&self) -> &'static str {
        "OPENAI_BASE_URL"
    }

    fn api_key_env(&self) -> &'static str {
        "OPENAI_API_KEY"
    }

    fn model_env(&self) -> Option<&'static str> {
        None
    }

    fn all_envs(&self) -> &'static [&'static str] {
        &["OPENAI_BASE_URL", "OPENAI_API_KEY"]
    }
}

impl ShellCapability for CodexTool {
    fn zsh_function_name(&self) -> Option<&'static str> {
        Some("codex")
    }
}

impl McpCapability for CodexTool {
    fn list_servers(&self) -> Result<Vec<McpServer>, ToolError> {
        list_servers_with_runner(self.target(), self.command(), &ProcessCommandRunner)
    }

    fn remove_server(&self, name: &str) -> Result<(), ToolError> {
        remove_server_with_runner(self.command(), name, &ProcessCommandRunner)
    }
}

impl PluginCapability for CodexTool {
    fn list_plugins(&self) -> Result<Vec<Plugin>, ToolError> {
        list_codex_plugins()
    }

    fn list_available_plugins(&self) -> Result<Vec<Plugin>, ToolError> {
        list_codex_available_plugins_with_runner(&ProcessCommandRunner)
    }

    fn install_plugin(&self, selector: &str) -> Result<(), ToolError> {
        install_codex_plugin_with_runner(selector, &ProcessCommandRunner)
    }

    fn set_plugin_enabled(&self, name: &str, enabled: bool) -> Result<(), ToolError> {
        set_codex_plugin_enabled(name, enabled)
    }

    fn remove_plugin(&self, name: &str) -> Result<(), ToolError> {
        remove_codex_plugin_with_runner(name, &ProcessCommandRunner)
    }
}
