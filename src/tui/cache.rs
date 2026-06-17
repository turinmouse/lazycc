use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::Target;
use crate::tools::{McpServer, Plugin};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct TuiCache {
    #[serde(default)]
    pub(crate) mcp_servers: Vec<McpServer>,
    #[serde(default)]
    pub(crate) installed_plugins: Vec<Plugin>,
}

impl TuiCache {
    pub(crate) fn load() -> Self {
        let path = cache_path();
        match fs::read_to_string(path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub(crate) fn save(&self) -> io::Result<()> {
        let path = cache_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, format!("{content}\n"))
    }

    pub(crate) fn replace_mcp_servers(&mut self, target: Target, mut servers: Vec<McpServer>) {
        self.mcp_servers.retain(|server| server.target != target);
        self.mcp_servers.append(&mut servers);
        self.mcp_servers.sort_by(|left, right| {
            left.target
                .to_string()
                .cmp(&right.target.to_string())
                .then_with(|| left.name.cmp(&right.name))
        });
    }

    pub(crate) fn replace_installed_plugins(&mut self, target: Target, mut plugins: Vec<Plugin>) {
        self.installed_plugins
            .retain(|plugin| plugin.target != target);
        self.installed_plugins.append(&mut plugins);
        sort_plugins(&mut self.installed_plugins);
    }
}

fn cache_path() -> PathBuf {
    std::env::temp_dir().join("lazycc-tui-cache.json")
}

fn sort_plugins(plugins: &mut [Plugin]) {
    plugins.sort_by(|left, right| {
        left.target
            .to_string()
            .cmp(&right.target.to_string())
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.selector.cmp(&right.selector))
    });
}
