use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Map, Value};

use crate::config::Target;
use crate::tools::{CommandRunner, ToolError};

const PLUGIN_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Plugin {
    pub(crate) target: Target,
    pub(crate) name: String,
    pub(crate) selector: String,
    pub(crate) marketplace: Option<String>,
    pub(crate) installed: bool,
    pub(crate) enabled: bool,
    pub(crate) details: String,
}

pub(crate) trait PluginCapability: Sync {
    fn list_plugins(&self) -> Result<Vec<Plugin>, ToolError>;
    fn list_available_plugins(&self) -> Result<Vec<Plugin>, ToolError>;
    fn install_plugin(&self, selector: &str) -> Result<(), ToolError>;
    fn set_plugin_enabled(&self, name: &str, enabled: bool) -> Result<(), ToolError>;
    fn remove_plugin(&self, name: &str) -> Result<(), ToolError>;
}

pub(crate) fn list_codex_plugins() -> Result<Vec<Plugin>, ToolError> {
    let path = codex_config_path()?;
    let content = read_optional_to_string(&path)?;
    let value = content
        .parse::<toml::Value>()
        .map_err(|error| ToolError::ConfigFailed {
            path: path.display().to_string(),
            source: error.to_string(),
        })?;

    let Some(plugins) = value.get("plugins").and_then(toml::Value::as_table) else {
        return Ok(Vec::new());
    };

    let mut result: Vec<Plugin> = plugins
        .iter()
        .map(|(name, config)| {
            let enabled = config
                .get("enabled")
                .and_then(toml::Value::as_bool)
                .unwrap_or(true);
            Plugin {
                target: Target::Codex,
                name: name.clone(),
                selector: name.clone(),
                marketplace: None,
                installed: true,
                enabled,
                details: format!("enabled: {enabled}"),
            }
        })
        .collect();
    result.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(result)
}

pub(crate) fn set_codex_plugin_enabled(name: &str, enabled: bool) -> Result<(), ToolError> {
    let path = codex_config_path()?;
    let content = read_optional_to_string(&path)?;
    let section = codex_plugin_section_header(name);
    let Some((start, end)) = find_toml_section(&content, &section) else {
        return Err(ToolError::ConfigFailed {
            path: path.display().to_string(),
            source: format!("plugin '{name}' was not found"),
        });
    };

    let mut lines: Vec<String> = content.lines().map(str::to_string).collect();
    let enabled_line = format!("enabled = {enabled}");
    if let Some(index) = (start + 1..end).find(|index| {
        lines[*index]
            .trim_start()
            .strip_prefix("enabled")
            .is_some_and(|rest| rest.trim_start().starts_with('='))
    }) {
        lines[index] = enabled_line;
    } else {
        lines.insert(start + 1, enabled_line);
    }

    write_lines(&path, &lines)
}

pub(crate) fn list_claude_plugins() -> Result<Vec<Plugin>, ToolError> {
    let path = claude_installed_plugins_path()?;
    let value = read_claude_plugins_json(&path)?;
    let plugins = value
        .get("plugins")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let enabled_plugins = value
        .get("enabledPlugins")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let mut result: Vec<Plugin> = plugins
        .iter()
        .map(|(name, installs)| {
            let enabled = enabled_plugins
                .get(name)
                .and_then(Value::as_bool)
                .unwrap_or(false);
            Plugin {
                target: Target::Claude,
                name: name.clone(),
                selector: name.clone(),
                marketplace: None,
                installed: true,
                enabled,
                details: claude_plugin_details(installs),
            }
        })
        .collect();
    result.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(result)
}

pub(crate) fn set_claude_plugin_enabled(name: &str, enabled: bool) -> Result<(), ToolError> {
    let path = claude_installed_plugins_path()?;
    let mut value = read_claude_plugins_json(&path)?;
    if !value
        .get("plugins")
        .and_then(Value::as_object)
        .is_some_and(|plugins| plugins.contains_key(name))
    {
        return Err(ToolError::ConfigFailed {
            path: path.display().to_string(),
            source: format!("plugin '{name}' was not found"),
        });
    }

    let root = value
        .as_object_mut()
        .ok_or_else(|| ToolError::ConfigFailed {
            path: path.display().to_string(),
            source: "expected JSON object".to_string(),
        })?;
    let enabled_plugins = root
        .entry("enabledPlugins")
        .or_insert_with(|| Value::Object(Map::new()));
    let enabled_plugins =
        enabled_plugins
            .as_object_mut()
            .ok_or_else(|| ToolError::ConfigFailed {
                path: path.display().to_string(),
                source: "expected enabledPlugins object".to_string(),
            })?;
    enabled_plugins.insert(name.to_string(), Value::Bool(enabled));

    write_json(&path, &value)
}

pub(crate) fn list_codex_available_plugins_with_runner(
    runner: &dyn CommandRunner,
) -> Result<Vec<Plugin>, ToolError> {
    list_available_plugins_with_runner(Target::Codex, "codex", runner)
}

pub(crate) fn list_claude_available_plugins_with_runner(
    runner: &dyn CommandRunner,
) -> Result<Vec<Plugin>, ToolError> {
    list_available_plugins_with_runner(Target::Claude, "claude", runner)
}

pub(crate) fn install_codex_plugin_with_runner(
    selector: &str,
    runner: &dyn CommandRunner,
) -> Result<(), ToolError> {
    run_plugin_command("codex", &["plugin", "add", selector, "--json"], runner)
}

pub(crate) fn install_claude_plugin_with_runner(
    selector: &str,
    runner: &dyn CommandRunner,
) -> Result<(), ToolError> {
    run_plugin_command("claude", &["plugin", "install", selector], runner)
}

pub(crate) fn remove_codex_plugin_with_runner(
    selector: &str,
    runner: &dyn CommandRunner,
) -> Result<(), ToolError> {
    run_plugin_command("codex", &["plugin", "remove", selector, "--json"], runner)
}

pub(crate) fn remove_claude_plugin_with_runner(
    selector: &str,
    runner: &dyn CommandRunner,
) -> Result<(), ToolError> {
    run_plugin_command("claude", &["plugin", "uninstall", selector, "-y"], runner)
}

fn codex_config_path() -> Result<PathBuf, ToolError> {
    home_path(".codex/config.toml")
}

fn claude_installed_plugins_path() -> Result<PathBuf, ToolError> {
    home_path(".claude/plugins/installed_plugins.json")
}

fn home_path(relative: &str) -> Result<PathBuf, ToolError> {
    dirs::home_dir()
        .map(|home| home.join(relative))
        .ok_or_else(|| ToolError::ConfigFailed {
            path: relative.to_string(),
            source: "home directory unavailable".to_string(),
        })
}

fn read_optional_to_string(path: &PathBuf) -> Result<String, ToolError> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(ToolError::ConfigFailed {
            path: path.display().to_string(),
            source: error.to_string(),
        }),
    }
}

fn read_claude_plugins_json(path: &PathBuf) -> Result<Value, ToolError> {
    let content = read_optional_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    serde_json::from_str(&content).map_err(|error| ToolError::ConfigFailed {
        path: path.display().to_string(),
        source: error.to_string(),
    })
}

fn write_json(path: &PathBuf, value: &Value) -> Result<(), ToolError> {
    let content = serde_json::to_string_pretty(value).map_err(|error| ToolError::ConfigFailed {
        path: path.display().to_string(),
        source: error.to_string(),
    })?;
    fs::write(path, format!("{content}\n")).map_err(|error| ToolError::ConfigFailed {
        path: path.display().to_string(),
        source: error.to_string(),
    })
}

fn write_lines(path: &PathBuf, lines: &[String]) -> Result<(), ToolError> {
    fs::write(path, format!("{}\n", lines.join("\n"))).map_err(|error| ToolError::ConfigFailed {
        path: path.display().to_string(),
        source: error.to_string(),
    })
}

fn codex_plugin_section_header(name: &str) -> String {
    format!("[plugins.\"{}\"]", name.replace('"', "\\\""))
}

fn find_toml_section(content: &str, header: &str) -> Option<(usize, usize)> {
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.iter().position(|line| line.trim() == header)?;
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(index, line)| line.trim_start().starts_with('[').then_some(index))
        .unwrap_or(lines.len());
    Some((start, end))
}

fn claude_plugin_details(installs: &Value) -> String {
    let Some(first) = installs.as_array().and_then(|items| items.first()) else {
        return "installed".to_string();
    };
    let version = first
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let scope = first
        .get("scope")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let path = first
        .get("installPath")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    format!("scope: {scope}, version: {version}, path: {path}")
}

fn list_available_plugins_with_runner(
    target: Target,
    command: &str,
    runner: &dyn CommandRunner,
) -> Result<Vec<Plugin>, ToolError> {
    let args = ["plugin", "list", "--json", "--available"];
    let output = runner.run_with_timeout(command, &args, PLUGIN_COMMAND_TIMEOUT)?;
    let command_line = format!("{command} {}", args.join(" "));
    if !output.success {
        return Err(ToolError::NonZeroExit {
            command: command_line,
            stdout: output.stdout,
            stderr: output.stderr,
        });
    }

    parse_available_plugins_output(target, &output.stdout).map_err(|error| {
        ToolError::ConfigFailed {
            path: command_line.clone(),
            source: error,
        }
    })
}

fn run_plugin_command(
    command: &str,
    args: &[&str],
    runner: &dyn CommandRunner,
) -> Result<(), ToolError> {
    let output = runner.run_with_timeout(command, args, PLUGIN_COMMAND_TIMEOUT)?;
    let command_line = format!("{command} {}", args.join(" "));
    if output.success {
        Ok(())
    } else {
        Err(ToolError::NonZeroExit {
            command: command_line,
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }
}

fn parse_available_plugins(target: Target, value: &Value) -> Vec<Plugin> {
    let mut plugins = Vec::new();
    if let Some(installed) = value.get("installed").and_then(Value::as_array) {
        plugins.extend(
            installed
                .iter()
                .filter_map(|value| parse_cli_plugin(target, value, true, target == Target::Codex)),
        );
    }
    if let Some(available) = value.get("available").and_then(Value::as_array) {
        plugins.extend(
            available
                .iter()
                .filter_map(|value| parse_cli_plugin(target, value, false, false)),
        );
    }

    plugins.sort_by(|left, right| {
        left.target
            .to_string()
            .cmp(&right.target.to_string())
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.selector.cmp(&right.selector))
    });
    plugins.dedup_by(|left, right| left.target == right.target && left.selector == right.selector);
    plugins
}

fn parse_available_plugins_output(target: Target, output: &str) -> Result<Vec<Plugin>, String> {
    match serde_json::from_str::<Value>(output) {
        Ok(value) => Ok(parse_available_plugins(target, &value)),
        Err(error) => {
            let plugins = parse_available_plugins_from_partial_json(target, output);
            if plugins.is_empty() {
                Err(error.to_string())
            } else {
                Ok(plugins)
            }
        }
    }
}

fn parse_available_plugins_from_partial_json(target: Target, output: &str) -> Vec<Plugin> {
    let mut plugins = Vec::new();
    plugins.extend(parse_plugin_array_from_partial_json(
        target,
        output,
        "installed",
        true,
        target == Target::Codex,
    ));
    plugins.extend(parse_plugin_array_from_partial_json(
        target,
        output,
        "available",
        false,
        false,
    ));
    plugins.sort_by(|left, right| {
        left.target
            .to_string()
            .cmp(&right.target.to_string())
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.selector.cmp(&right.selector))
    });
    plugins.dedup_by(|left, right| left.target == right.target && left.selector == right.selector);
    plugins
}

fn parse_plugin_array_from_partial_json(
    target: Target,
    output: &str,
    key: &str,
    default_installed: bool,
    default_enabled: bool,
) -> Vec<Plugin> {
    let Some(array_start) = output.find(&format!("\"{key}\"")).and_then(|key_start| {
        output[key_start..]
            .find('[')
            .map(|offset| key_start + offset + 1)
    }) else {
        return Vec::new();
    };

    let mut result = Vec::new();
    let mut object_start = None;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (offset, value) in output[array_start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if value == '\\' {
                escaped = true;
            } else if value == '"' {
                in_string = false;
            }
            continue;
        }

        match value {
            '"' => in_string = true,
            '{' => {
                if depth == 0 {
                    object_start = Some(array_start + offset);
                }
                depth += 1;
            }
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0
                    && let Some(start) = object_start.take()
                {
                    let end = array_start + offset + value.len_utf8();
                    if let Ok(value) = serde_json::from_str::<Value>(&output[start..end])
                        && let Some(plugin) =
                            parse_cli_plugin(target, &value, default_installed, default_enabled)
                    {
                        result.push(plugin);
                    }
                }
            }
            ']' if depth == 0 => break,
            _ => {}
        }
    }

    result
}

fn parse_cli_plugin(
    target: Target,
    value: &Value,
    default_installed: bool,
    default_enabled: bool,
) -> Option<Plugin> {
    let raw_name = string_field(value, &["name", "id", "pluginId", "plugin_id"])?;
    let marketplace = string_field(
        value,
        &["marketplaceName", "marketplace", "marketplace_name"],
    )
    .or_else(|| {
        raw_name
            .split_once('@')
            .map(|(_, marketplace)| marketplace.to_string())
    });
    let name = string_field(value, &["name"]).unwrap_or_else(|| {
        raw_name
            .split_once('@')
            .map(|(name, _)| name.to_string())
            .unwrap_or(raw_name)
    });
    let selector = string_field(value, &["pluginId", "plugin_id", "id"]).unwrap_or_else(|| {
        marketplace
            .as_ref()
            .map(|marketplace| format!("{name}@{marketplace}"))
            .unwrap_or_else(|| name.clone())
    });
    let installed = value
        .get("installed")
        .and_then(Value::as_bool)
        .unwrap_or(default_installed);
    let enabled = value
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(default_enabled && installed);
    let version = string_field(value, &["version"]);
    let source = string_field(value, &["source", "marketplaceSource", "description"]);
    let details = [
        marketplace.as_deref(),
        version.as_deref(),
        source.as_deref(),
    ]
    .into_iter()
    .flatten()
    .filter(|part| !part.is_empty())
    .collect::<Vec<_>>()
    .join(" | ");

    Some(Plugin {
        target,
        name,
        selector,
        marketplace,
        installed,
        enabled,
        details,
    })
}

fn string_field(value: &Value, fields: &[&str]) -> Option<String> {
    fields.iter().find_map(|field| {
        value
            .get(*field)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::runner::CommandOutput;

    struct FakeRunner {
        output: CommandOutput,
        expected_command: &'static str,
        expected_args: Vec<&'static str>,
    }

    impl CommandRunner for FakeRunner {
        fn run(&self, command: &str, args: &[&str]) -> Result<CommandOutput, ToolError> {
            assert_eq!(command, self.expected_command);
            assert_eq!(args, self.expected_args.as_slice());
            Ok(self.output.clone())
        }
    }

    #[test]
    fn parses_codex_available_plugins() {
        let value = serde_json::json!({
            "installed": [{
                "name": "context-mode",
                "pluginId": "context-mode@main",
                "marketplaceName": "main",
                "enabled": true,
                "installed": true,
                "version": "1.0.0"
            }],
            "available": [{
                "name": "review",
                "pluginId": "review@main",
                "marketplaceName": "main",
                "version": "0.2.0"
            }]
        });

        let plugins = parse_available_plugins(Target::Codex, &value);

        assert_eq!(plugins.len(), 2);
        assert_eq!(plugins[0].name, "context-mode");
        assert_eq!(plugins[0].selector, "context-mode@main");
        assert!(plugins[0].installed);
        assert!(plugins[0].enabled);
        assert_eq!(plugins[1].selector, "review@main");
        assert!(!plugins[1].installed);
    }

    #[test]
    fn list_available_uses_codex_cli_json_available_order() {
        let runner = FakeRunner {
            output: CommandOutput {
                success: true,
                stdout:
                    r#"{"installed":[],"available":[{"name":"sample","pluginId":"sample@debug"}]}"#
                        .to_string(),
                stderr: String::new(),
            },
            expected_command: "codex",
            expected_args: vec!["plugin", "list", "--json", "--available"],
        };

        let plugins = list_codex_available_plugins_with_runner(&runner)
            .expect("fake JSON output should parse");

        assert_eq!(plugins[0].selector, "sample@debug");
    }

    #[test]
    fn parses_real_codex_plugin_list_shape() {
        let value = serde_json::json!({
            "installed": [{
                "pluginId": "context-mode@context-mode",
                "name": "context-mode",
                "marketplaceName": "context-mode",
                "version": "1.0.162",
                "installed": true,
                "enabled": true,
                "source": {
                    "source": "git",
                    "url": "/Users/test/.codex/.tmp/marketplaces/context-mode"
                },
                "marketplaceSource": {
                    "sourceType": "git",
                    "source": "https://github.com/mksglu/context-mode.git"
                },
                "installPolicy": "AVAILABLE",
                "authPolicy": "ON_INSTALL"
            }],
            "available": [{
                "pluginId": "linear@openai-curated",
                "name": "linear",
                "marketplaceName": "openai-curated",
                "version": "0.0.2",
                "installed": false,
                "enabled": false,
                "source": {
                    "source": "local",
                    "path": "/Users/test/.codex/.tmp/plugins/plugins/linear"
                },
                "installPolicy": "AVAILABLE",
                "authPolicy": "ON_INSTALL"
            }]
        });

        let plugins = parse_available_plugins(Target::Codex, &value);

        assert_eq!(
            plugins
                .iter()
                .map(|plugin| plugin.selector.as_str())
                .collect::<Vec<_>>(),
            vec!["context-mode@context-mode", "linear@openai-curated"]
        );
        assert!(plugins[0].installed);
        assert!(!plugins[1].installed);
    }

    #[test]
    fn parses_real_claude_plugin_list_shape() {
        let value = serde_json::json!({
            "installed": [{
                "id": "context-mode@context-mode",
                "version": "1.0.146",
                "scope": "user",
                "enabled": true,
                "installPath": "/Users/test/.claude/plugins/cache/context-mode/context-mode/1.0.146",
                "mcpServers": {
                    "context-mode": {
                        "command": "node",
                        "args": ["${CLAUDE_PLUGIN_ROOT}/start.mjs"]
                    }
                }
            }],
            "available": [{
                "pluginId": "42crunch-api-security-testing@claude-plugins-official",
                "name": "42crunch-api-security-testing",
                "description": "API security testing tools",
                "version": "1.5.5",
                "marketplaceName": "claude-plugins-official",
                "source": {
                    "source": "git-subdir",
                    "url": "https://github.com/42Crunch-AI/claude-plugins.git",
                    "path": "plugins/api-security-testing",
                    "ref": "v1.5.5",
                    "sha": "c2951754af2b811955957d22716844b5a3c016e9"
                },
                "installCount": 872
            }]
        });

        let plugins = parse_available_plugins(Target::Claude, &value);

        assert_eq!(plugins[0].name, "42crunch-api-security-testing");
        assert_eq!(
            plugins[0].selector,
            "42crunch-api-security-testing@claude-plugins-official"
        );
        assert_eq!(
            plugins[0].marketplace.as_deref(),
            Some("claude-plugins-official")
        );
        assert!(!plugins[0].installed);
        assert_eq!(plugins[1].name, "context-mode");
        assert_eq!(plugins[1].selector, "context-mode@context-mode");
        assert_eq!(plugins[1].marketplace.as_deref(), Some("context-mode"));
        assert!(plugins[1].installed);
        assert!(plugins[1].enabled);
    }

    #[test]
    fn recovers_complete_claude_plugins_from_truncated_json() {
        let output = r#"{
          "installed": [
            {
              "id": "context-mode@context-mode",
              "version": "1.0.146",
              "scope": "user",
              "enabled": true
            }
          ],
          "available": [
            {
              "pluginId": "linear@claude-plugins-official",
              "name": "linear",
              "description": "Linear tools",
              "version": "1.0.0",
              "marketplaceName": "claude-plugins-official"
            },
            {
              "pluginId": "mapbox@claude-plugins-official",
              "name": "mapbox",
              "description": "unterminated
        "#;

        let plugins = parse_available_plugins_output(Target::Claude, output)
            .expect("complete objects before the truncated item should be recovered");

        assert_eq!(
            plugins
                .iter()
                .map(|plugin| plugin.selector.as_str())
                .collect::<Vec<_>>(),
            vec![
                "context-mode@context-mode",
                "linear@claude-plugins-official"
            ]
        );
    }

    #[test]
    fn install_and_remove_use_target_cli_commands() {
        let install = FakeRunner {
            output: CommandOutput {
                success: true,
                stdout: String::new(),
                stderr: String::new(),
            },
            expected_command: "claude",
            expected_args: vec!["plugin", "install", "sample@debug"],
        };
        install_claude_plugin_with_runner("sample@debug", &install).expect("install should work");

        let remove = FakeRunner {
            output: CommandOutput {
                success: true,
                stdout: String::new(),
                stderr: String::new(),
            },
            expected_command: "codex",
            expected_args: vec!["plugin", "remove", "sample@debug", "--json"],
        };
        remove_codex_plugin_with_runner("sample@debug", &remove).expect("remove should work");
    }
}
