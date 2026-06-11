use crate::config::Target;
use crate::tools::{CommandRunner, ToolError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct McpServer {
    pub(crate) target: Target,
    pub(crate) name: String,
    pub(crate) details: String,
}

pub(crate) trait McpCapability: Sync {
    fn list_servers(&self) -> Result<Vec<McpServer>, ToolError>;
    fn remove_server(&self, name: &str) -> Result<(), ToolError>;
}

pub(crate) fn list_servers_with_runner(
    target: Target,
    command: &str,
    runner: &dyn CommandRunner,
) -> Result<Vec<McpServer>, ToolError> {
    let output = runner.run(command, &["mcp", "list"])?;
    let command_line = format!("{command} mcp list");
    if !output.success {
        return Err(ToolError::NonZeroExit {
            command: command_line,
            stdout: output.stdout,
            stderr: output.stderr,
        });
    }

    Ok(parse_mcp_list(
        target,
        &format!("{}\n{}", output.stdout, output.stderr),
    ))
}

pub(crate) fn remove_server_with_runner(
    command: &str,
    name: &str,
    runner: &dyn CommandRunner,
) -> Result<(), ToolError> {
    let output = runner.run(command, &["mcp", "remove", name])?;
    let command_line = format!("{command} mcp remove {name}");
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

pub(crate) fn parse_mcp_list(target: Target, output: &str) -> Vec<McpServer> {
    output
        .lines()
        .filter_map(|line| parse_mcp_line(target, line))
        .collect()
}

fn parse_mcp_line(target: Target, line: &str) -> Option<McpServer> {
    let trimmed = line.trim();
    if trimmed.is_empty()
        || trimmed.starts_with("WARNING:")
        || trimmed.starts_with("Checking ")
        || trimmed.starts_with("Name ")
    {
        return None;
    }

    let name = if target == Target::Claude {
        trimmed.rsplit_once(": ")?.0.trim()
    } else {
        trimmed.split_whitespace().next()?
    };

    Some(McpServer {
        target,
        name: name.to_string(),
        details: trimmed.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::runner::CommandOutput;

    struct FakeRunner {
        output: CommandOutput,
        expected_args: Vec<&'static str>,
    }

    impl CommandRunner for FakeRunner {
        fn run(&self, command: &str, args: &[&str]) -> Result<CommandOutput, ToolError> {
            assert_eq!(command, "codex");
            assert_eq!(args, self.expected_args.as_slice());
            Ok(self.output.clone())
        }
    }

    #[test]
    fn codex_list_parser_skips_noise_and_uses_first_column_as_name() {
        let servers = parse_mcp_list(
            Target::Codex,
            r#"
                WARNING: experimental
                Name Command Args
                context-mode node ./start.mjs

                github npx -y @modelcontextprotocol/server-github
            "#,
        );

        assert_eq!(
            servers,
            vec![
                McpServer {
                    target: Target::Codex,
                    name: "context-mode".to_string(),
                    details: "context-mode node ./start.mjs".to_string(),
                },
                McpServer {
                    target: Target::Codex,
                    name: "github".to_string(),
                    details: "github npx -y @modelcontextprotocol/server-github".to_string(),
                },
            ]
        );
    }

    #[test]
    fn claude_list_parser_uses_colon_prefix_as_name() {
        let servers = parse_mcp_list(
            Target::Claude,
            r#"
                Checking MCP server health...
                context-mode: node ./start.mjs
                github: npx -y @modelcontextprotocol/server-github
            "#,
        );

        assert_eq!(
            servers
                .iter()
                .map(|server| server.name.as_str())
                .collect::<Vec<_>>(),
            vec!["context-mode", "github"]
        );
    }

    #[test]
    fn claude_list_parser_keeps_names_with_colons_and_health_status() {
        let servers = parse_mcp_list(
            Target::Claude,
            r#"
                Checking MCP server health…

                plugin:context-mode:context-mode: node /Users/test/start.mjs - ✓ Connected
                serena: serena start-mcp-server --context=claude-code - ✗ Failed to connect
                openspace: openspace-mcp  - ✗ Failed to connect
            "#,
        );

        assert_eq!(
            servers
                .iter()
                .map(|server| server.name.as_str())
                .collect::<Vec<_>>(),
            vec!["plugin:context-mode:context-mode", "serena", "openspace"]
        );
    }

    #[test]
    fn list_uses_runner_command_shape() {
        let runner = FakeRunner {
            output: CommandOutput {
                success: true,
                stdout: "context-mode node ./start.mjs\n".to_string(),
                stderr: String::new(),
            },
            expected_args: vec!["mcp", "list"],
        };

        let servers = list_servers_with_runner(Target::Codex, "codex", &runner)
            .expect("list should parse fake output");

        assert_eq!(servers[0].name, "context-mode");
    }

    #[test]
    fn remove_uses_runner_command_shape() {
        let runner = FakeRunner {
            output: CommandOutput {
                success: true,
                stdout: String::new(),
                stderr: String::new(),
            },
            expected_args: vec!["mcp", "remove", "context-mode"],
        };

        remove_server_with_runner("codex", "context-mode", &runner)
            .expect("remove should accept fake successful output");
    }
}
