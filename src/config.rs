use std::fmt;
use std::fs;
use std::fs::OpenOptions;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use clap::ValueEnum;
use comfy_table::{Cell, CellAlignment, Table, presets::UTF8_FULL};
use inquire::Select;
use serde::{Deserialize, Serialize};

use crate::error::LazyccError;
use crate::template::render_template;
use crate::tools::{all_tools, tool_for, tool_sort_key};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum Shell {
    Zsh,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Target {
    Codex,
    Claude,
}

impl Target {
    pub(crate) fn prompt() -> Result<Self, LazyccError> {
        Select::new("Target:", vec![Target::Codex, Target::Claude])
            .prompt()
            .map_err(LazyccError::from)
    }

    pub(crate) fn all() -> [Target; 2] {
        [Target::Codex, Target::Claude]
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Target::Codex => write!(f, "codex"),
            Target::Claude => write!(f, "claude"),
        }
    }
}

impl FromStr for Target {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "codex" => Ok(Target::Codex),
            "claude" => Ok(Target::Claude),
            _ => Err(format!("unsupported target '{value}'")),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct CurrentProfile {
    pub(crate) target: Target,
    pub(crate) name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Profile {
    pub(crate) name: String,
    pub(crate) target: Target,
    pub(crate) base_url: String,
    pub(crate) api_key: String,
    #[serde(default)]
    pub(crate) model: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct Config {
    #[serde(default = "default_current_profiles")]
    pub(crate) current: Vec<CurrentProfile>,
    #[serde(default)]
    pub(crate) profiles: Vec<Profile>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            current: default_current_profiles(),
            profiles: default_profiles(),
        }
    }
}

impl Config {
    pub(crate) fn load(path: &Path) -> Result<Self, LazyccError> {
        let config = match fs::read_to_string(path) {
            Ok(content) => toml::from_str(&content).map_err(LazyccError::from),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(LazyccError::from(error)),
        }?;

        Ok(config.with_default_profiles())
    }

    pub(crate) fn save(&self, path: &Path) -> Result<(), LazyccError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)?;
        file.write_all(content.as_bytes())?;
        set_config_permissions(path)?;
        Ok(())
    }

    pub(crate) fn add(&mut self, profile: Profile) -> Result<(), LazyccError> {
        if self
            .profiles
            .iter()
            .any(|existing| existing.name == profile.name && existing.target == profile.target)
        {
            return Err(LazyccError::DuplicateProfile {
                target: profile.target,
                name: profile.name,
            });
        }

        self.profiles.push(profile);
        Ok(())
    }

    pub(crate) fn delete(&mut self, name: &str, target: Option<Target>) -> Result<(), LazyccError> {
        let resolved = self.resolve(name, target)?;
        self.profiles
            .retain(|profile| profile.name != resolved.name || profile.target != resolved.target);

        if self.is_current(&resolved) {
            self.set_current(default_current_profile(resolved.target));
        }
        self.ensure_default_profiles();
        self.ensure_default_current();

        Ok(())
    }

    pub(crate) fn use_profile(
        &mut self,
        name: &str,
        target: Option<Target>,
    ) -> Result<(), LazyccError> {
        let resolved = self.resolve(name, target)?;
        self.set_current(resolved);
        Ok(())
    }

    fn resolve(&self, name: &str, target: Option<Target>) -> Result<CurrentProfile, LazyccError> {
        let matches: Vec<&Profile> = self
            .profiles
            .iter()
            .filter(|profile| {
                profile.name == name && target.is_none_or(|target| profile.target == target)
            })
            .collect();

        match matches.as_slice() {
            [] => Err(LazyccError::ProfileNotFound {
                target,
                name: name.to_string(),
            }),
            [profile] => Ok(CurrentProfile {
                target: profile.target,
                name: profile.name.clone(),
            }),
            _ => Err(LazyccError::AmbiguousProfile {
                name: name.to_string(),
            }),
        }
    }

    fn current_profiles(&self) -> Vec<&Profile> {
        self.current
            .iter()
            .filter_map(|current| {
                self.profiles.iter().find(|profile| {
                    profile.name == current.name && profile.target == current.target
                })
            })
            .collect()
    }

    pub(crate) fn init_script(&self, shell: Shell) -> String {
        match shell {
            Shell::Zsh => self.zsh_init_script(),
        }
    }

    fn zsh_init_script(&self) -> String {
        render_template(SHELL_ZSH_TEMPLATE, self.zsh_init_context())
    }

    fn zsh_init_context(&self) -> ShellInitContext {
        let mut function_names = vec!["lazycc".to_string()];
        function_names.extend(all_tools().iter().filter_map(|tool| {
            tool.shell()
                .and_then(|shell| shell.zsh_function_name())
                .map(str::to_string)
        }));

        let unset_envs = all_tools()
            .iter()
            .flat_map(|tool| tool.env().all_envs())
            .map(|name| (*name).to_string())
            .collect();

        let mut exports = Vec::new();
        let mut codex_wrapper = None;
        for profile in self.current_profiles() {
            let tool = tool_for(profile.target);
            push_export_if_present(&mut exports, tool.env().base_url_env(), &profile.base_url);
            push_export_if_present(&mut exports, tool.env().api_key_env(), &profile.api_key);
            if let Some(model_key) = tool.env().model_env() {
                push_export_if_present(&mut exports, model_key, &profile.model);
            }

            if profile.target == Target::Codex && profile.name != DEFAULT_CODEX_PROFILE {
                codex_wrapper = Some(CodexWrapperContext::from_profile(profile));
            }
        }

        ShellInitContext {
            function_names,
            unset_envs,
            exports,
            codex_wrapper,
        }
    }

    pub(crate) fn render_table(&self) -> String {
        let mut table = Table::new();
        table.load_preset(UTF8_FULL);
        table.set_header(vec![
            centered_cell("CURRENT"),
            centered_cell("NAME"),
            centered_cell("TARGET"),
            centered_cell("MODEL"),
            centered_cell("BASE_URL"),
            centered_cell("API_KEY"),
        ]);

        let mut profiles: Vec<&Profile> = self.profiles.iter().collect();
        profiles.sort_by(|left, right| {
            tool_sort_key(left.target)
                .cmp(&tool_sort_key(right.target))
                .then_with(|| left.name.cmp(&right.name))
        });

        for profile in profiles {
            let is_current = self
                .current
                .iter()
                .any(|current| current.name == profile.name && current.target == profile.target);
            table.add_row(vec![
                centered_cell(if is_current { "*" } else { "" }),
                centered_cell(&profile.name),
                centered_cell(tool_for(profile.target).display_name()),
                centered_cell(&profile.model),
                centered_cell(&profile.base_url),
                centered_cell(mask_api_key(&profile.api_key)),
            ]);
        }

        table.to_string()
    }

    fn with_default_profiles(mut self) -> Self {
        self.ensure_default_profiles();
        self.ensure_default_current();
        self
    }

    fn ensure_default_profiles(&mut self) {
        for default_profile in default_profiles() {
            if !self.profiles.iter().any(|profile| {
                profile.name == default_profile.name && profile.target == default_profile.target
            }) {
                self.profiles.push(default_profile);
            }
        }
    }

    fn ensure_default_current(&mut self) {
        for target in Target::all() {
            if !self.current.iter().any(|current| current.target == target) {
                self.current.push(default_current_profile(target));
            }
        }
    }

    pub(crate) fn is_current(&self, profile: &CurrentProfile) -> bool {
        self.current_for_target(profile.target)
            .is_some_and(|current| current.name == profile.name)
    }

    pub(crate) fn current_for_target(&self, target: Target) -> Option<&CurrentProfile> {
        self.current.iter().find(|current| current.target == target)
    }

    pub(crate) fn set_current(&mut self, profile: CurrentProfile) {
        self.current
            .retain(|current| current.target != profile.target);
        self.current.push(profile);
    }
}

pub(crate) const DEFAULT_CODEX_PROFILE: &str = "openai";
pub(crate) const DEFAULT_CLAUDE_PROFILE: &str = "anthropic";

pub(crate) fn default_current_profiles() -> Vec<CurrentProfile> {
    Target::all()
        .into_iter()
        .map(default_current_profile)
        .collect()
}

pub(crate) fn default_current_profile(target: Target) -> CurrentProfile {
    let name = match target {
        Target::Codex => DEFAULT_CODEX_PROFILE,
        Target::Claude => DEFAULT_CLAUDE_PROFILE,
    };

    CurrentProfile {
        target,
        name: name.to_string(),
    }
}

pub(crate) fn default_profiles() -> Vec<Profile> {
    vec![
        Profile {
            name: DEFAULT_CODEX_PROFILE.to_string(),
            target: Target::Codex,
            base_url: String::new(),
            api_key: String::new(),
            model: String::new(),
        },
        Profile {
            name: DEFAULT_CLAUDE_PROFILE.to_string(),
            target: Target::Claude,
            base_url: String::new(),
            api_key: String::new(),
            model: String::new(),
        },
    ]
}

pub(crate) fn config_path() -> Result<PathBuf, LazyccError> {
    config_path_from(std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
}

pub(crate) fn config_path_from(xdg_config_home: Option<PathBuf>) -> Result<PathBuf, LazyccError> {
    xdg_config_home
        .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
        .map(|path| path.join("lazycc").join("config.toml"))
        .ok_or(LazyccError::ConfigDirUnavailable)
}

#[cfg(unix)]
fn set_config_permissions(path: &Path) -> Result<(), LazyccError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_config_permissions(_path: &Path) -> Result<(), LazyccError> {
    Ok(())
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }

    format!("'{}'", value.replace('\'', "'\\''"))
}

fn push_export_if_present(exports: &mut Vec<ShellExport>, name: &str, value: &str) {
    if value.is_empty() {
        return;
    }

    exports.push(ShellExport {
        name: name.to_string(),
        value: shell_quote(value),
    });
}

fn centered_cell(value: impl ToString) -> Cell {
    Cell::new(value).set_alignment(CellAlignment::Center)
}

const SHELL_ZSH_TEMPLATE: &str = include_str!("templates/shell.zsh");

#[derive(Serialize)]
struct ShellInitContext {
    function_names: Vec<String>,
    unset_envs: Vec<String>,
    exports: Vec<ShellExport>,
    codex_wrapper: Option<CodexWrapperContext>,
}

#[derive(Serialize)]
struct ShellExport {
    name: String,
    value: String,
}

#[derive(Serialize)]
struct CodexWrapperContext {
    config_args: Vec<String>,
}

impl CodexWrapperContext {
    fn from_profile(profile: &Profile) -> Self {
        let mut config_args = vec![shell_quote(&format!("model_provider={}", profile.name))];
        if !profile.model.is_empty() {
            config_args.push(shell_quote(&format!("model={}", profile.model)));
        }
        config_args.extend([
            shell_quote(&format!(
                "model_providers.{}.name={}",
                profile.name, profile.name
            )),
            format!(
                "{}\"${{OPENAI_BASE_URL}}\"",
                shell_quote(&format!("model_providers.{}.base_url=", profile.name))
            ),
            shell_quote(&format!(
                "model_providers.{}.env_key=OPENAI_API_KEY",
                profile.name
            )),
            shell_quote(&format!(
                "model_providers.{}.wire_api=responses",
                profile.name
            )),
        ]);

        Self { config_args }
    }
}

pub(crate) fn mask_api_key(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    match chars.len() {
        0 => String::new(),
        1..=8 => "*".repeat(chars.len()),
        len => {
            let prefix: String = chars.iter().take(4).collect();
            let suffix: String = chars.iter().skip(len - 4).collect();
            format!("{prefix}{}{suffix}", "*".repeat(len - 8))
        }
    }
}
