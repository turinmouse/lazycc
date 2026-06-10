use std::fmt;
use std::fs;
use std::fs::OpenOptions;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use clap::{Parser, Subcommand, ValueEnum};
use comfy_table::{Cell, Table, presets::UTF8_FULL};
use inquire::{Password, PasswordDisplayMode, Select, Text};
use serde::{Deserialize, Serialize};

fn main() {
    if let Err(error) = run() {
        eprintln!("capm: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), CapmError> {
    let cli = Cli::parse();
    let path = config_path()?;

    match cli.command {
        Command::Init { shell } => {
            let config = Config::load(&path)?;
            print!("{}", config.init_script(shell));
        }
        Command::List => {
            let config = Config::load(&path)?;
            println!("{}", config.render_table());
        }
        Command::Add { name, target } => {
            let target = match target {
                Some(target) => target,
                None => Target::prompt()?,
            };
            let base_url = Text::new("Base URL:").prompt()?;
            let api_key = Password::new("API key:")
                .with_display_mode(PasswordDisplayMode::Masked)
                .without_confirmation()
                .prompt()?;

            let mut config = Config::load(&path)?;
            config.add(Profile {
                name,
                target,
                base_url,
                api_key,
            })?;
            config.save(&path)?;
        }
        Command::Del { name, target } => {
            let mut config = Config::load(&path)?;
            config.delete(&name, target)?;
            config.save(&path)?;
        }
        Command::Switch { name, target } => {
            let mut config = Config::load(&path)?;
            config.switch(&name, target)?;
            config.save(&path)?;
        }
    }

    Ok(())
}

#[derive(Parser, Debug)]
#[command(
    name = "capm",
    version,
    about = "Manage coding-agent API provider profiles"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Init {
        shell: Shell,
    },
    List,
    Add {
        name: String,
        #[arg(long)]
        target: Option<Target>,
    },
    Del {
        name: String,
        #[arg(long)]
        target: Option<Target>,
    },
    Switch {
        name: String,
        #[arg(long)]
        target: Option<Target>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum Shell {
    Zsh,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
enum Target {
    Codex,
    Claude,
}

impl Target {
    fn prompt() -> Result<Self, CapmError> {
        Select::new("Target:", vec![Target::Codex, Target::Claude])
            .prompt()
            .map_err(CapmError::from)
    }

    fn display_name(self) -> &'static str {
        match self {
            Target::Codex => "Codex",
            Target::Claude => "Claude Code",
        }
    }

    fn env_vars(self) -> (&'static str, &'static str) {
        match self {
            Target::Codex => ("OPENAI_BASE_URL", "OPENAI_API_KEY"),
            Target::Claude => ("ANTHROPIC_BASE_URL", "ANTHROPIC_AUTH_TOKEN"),
        }
    }

    fn all_env_vars() -> &'static [&'static str] {
        &[
            "OPENAI_BASE_URL",
            "OPENAI_API_KEY",
            "ANTHROPIC_BASE_URL",
            "ANTHROPIC_AUTH_TOKEN",
        ]
    }

    fn all() -> [Target; 2] {
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
struct CurrentProfile {
    target: Target,
    name: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct Profile {
    name: String,
    target: Target,
    base_url: String,
    api_key: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct Config {
    #[serde(default = "default_current_profiles")]
    current: Vec<CurrentProfile>,
    #[serde(default)]
    profiles: Vec<Profile>,
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
    fn load(path: &Path) -> Result<Self, CapmError> {
        let config = match fs::read_to_string(path) {
            Ok(content) => toml::from_str(&content).map_err(CapmError::from),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(CapmError::from(error)),
        }?;

        Ok(config.with_default_profiles())
    }

    fn save(&self, path: &Path) -> Result<(), CapmError> {
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

    fn add(&mut self, profile: Profile) -> Result<(), CapmError> {
        if self
            .profiles
            .iter()
            .any(|existing| existing.name == profile.name && existing.target == profile.target)
        {
            return Err(CapmError::DuplicateProfile {
                target: profile.target,
                name: profile.name,
            });
        }

        self.profiles.push(profile);
        Ok(())
    }

    fn delete(&mut self, name: &str, target: Option<Target>) -> Result<(), CapmError> {
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

    fn switch(&mut self, name: &str, target: Option<Target>) -> Result<(), CapmError> {
        let resolved = self.resolve(name, target)?;
        self.set_current(resolved);
        Ok(())
    }

    fn resolve(&self, name: &str, target: Option<Target>) -> Result<CurrentProfile, CapmError> {
        let matches: Vec<&Profile> = self
            .profiles
            .iter()
            .filter(|profile| {
                profile.name == name && target.is_none_or(|target| profile.target == target)
            })
            .collect();

        match matches.as_slice() {
            [] => Err(CapmError::ProfileNotFound {
                target,
                name: name.to_string(),
            }),
            [profile] => Ok(CurrentProfile {
                target: profile.target,
                name: profile.name.clone(),
            }),
            _ => Err(CapmError::AmbiguousProfile {
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

    fn init_script(&self, shell: Shell) -> String {
        match shell {
            Shell::Zsh => self.zsh_init_script(),
        }
    }

    fn zsh_init_script(&self) -> String {
        let mut output = String::new();
        for name in Target::all_env_vars() {
            output.push_str("unset ");
            output.push_str(name);
            output.push('\n');
        }

        for profile in self.current_profiles() {
            let (base_url_key, api_key_key) = profile.target.env_vars();
            push_export_if_present(&mut output, base_url_key, &profile.base_url);
            push_export_if_present(&mut output, api_key_key, &profile.api_key);
        }

        output
    }

    fn render_table(&self) -> String {
        let mut table = Table::new();
        table.load_preset(UTF8_FULL);
        table.set_header(vec!["CURRENT", "NAME", "TARGET", "BASE_URL", "API_KEY"]);

        for profile in &self.profiles {
            let is_current = self
                .current
                .iter()
                .any(|current| current.name == profile.name && current.target == profile.target);
            table.add_row(vec![
                Cell::new(if is_current { "*" } else { "" }),
                Cell::new(&profile.name),
                Cell::new(profile.target.display_name()),
                Cell::new(&profile.base_url),
                Cell::new(mask_api_key(&profile.api_key)),
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

    fn is_current(&self, profile: &CurrentProfile) -> bool {
        self.current_for_target(profile.target)
            .is_some_and(|current| current.name == profile.name)
    }

    fn current_for_target(&self, target: Target) -> Option<&CurrentProfile> {
        self.current.iter().find(|current| current.target == target)
    }

    fn set_current(&mut self, profile: CurrentProfile) {
        self.current
            .retain(|current| current.target != profile.target);
        self.current.push(profile);
    }
}

fn default_current_profiles() -> Vec<CurrentProfile> {
    Target::all()
        .into_iter()
        .map(default_current_profile)
        .collect()
}

fn default_current_profile(target: Target) -> CurrentProfile {
    let name = match target {
        Target::Codex => "openai",
        Target::Claude => "anthropic",
    };

    CurrentProfile {
        target,
        name: name.to_string(),
    }
}

fn default_profiles() -> Vec<Profile> {
    vec![
        Profile {
            name: "openai".to_string(),
            target: Target::Codex,
            base_url: String::new(),
            api_key: String::new(),
        },
        Profile {
            name: "anthropic".to_string(),
            target: Target::Claude,
            base_url: String::new(),
            api_key: String::new(),
        },
    ]
}

fn config_path() -> Result<PathBuf, CapmError> {
    config_path_from(std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from))
}

fn config_path_from(xdg_config_home: Option<PathBuf>) -> Result<PathBuf, CapmError> {
    xdg_config_home
        .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
        .map(|path| path.join("capm").join("config.toml"))
        .ok_or(CapmError::ConfigDirUnavailable)
}

#[cfg(unix)]
fn set_config_permissions(path: &Path) -> Result<(), CapmError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_config_permissions(_path: &Path) -> Result<(), CapmError> {
    Ok(())
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }

    format!("'{}'", value.replace('\'', "'\\''"))
}

fn push_export_if_present(output: &mut String, name: &str, value: &str) {
    if value.is_empty() {
        return;
    }

    output.push_str("export ");
    output.push_str(name);
    output.push('=');
    output.push_str(&shell_quote(value));
    output.push('\n');
}

fn mask_api_key(value: &str) -> String {
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

#[derive(Debug)]
enum CapmError {
    AmbiguousProfile {
        name: String,
    },
    ConfigDirUnavailable,
    DuplicateProfile {
        target: Target,
        name: String,
    },
    Io(io::Error),
    ProfileNotFound {
        target: Option<Target>,
        name: String,
    },
    Prompt(inquire::error::InquireError),
    TomlDeserialize(toml::de::Error),
    TomlSerialize(toml::ser::Error),
}

impl fmt::Display for CapmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CapmError::AmbiguousProfile { name } => {
                write!(
                    f,
                    "profile '{name}' exists for multiple targets; pass --target"
                )
            }
            CapmError::ConfigDirUnavailable => {
                write!(f, "could not determine user config directory")
            }
            CapmError::DuplicateProfile { target, name } => {
                write!(f, "profile '{name}' already exists for target '{target}'")
            }
            CapmError::Io(error) => write!(f, "{error}"),
            CapmError::ProfileNotFound { target, name } => match target {
                Some(target) => write!(f, "profile '{name}' for target '{target}' was not found"),
                None => write!(f, "profile '{name}' was not found"),
            },
            CapmError::Prompt(error) => write!(f, "{error}"),
            CapmError::TomlDeserialize(error) => write!(f, "{error}"),
            CapmError::TomlSerialize(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for CapmError {}

impl From<io::Error> for CapmError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<inquire::error::InquireError> for CapmError {
    fn from(error: inquire::error::InquireError) -> Self {
        Self::Prompt(error)
    }
}

impl From<toml::de::Error> for CapmError {
    fn from(error: toml::de::Error) -> Self {
        Self::TomlDeserialize(error)
    }
}

impl From<toml::ser::Error> for CapmError {
    fn from(error: toml::ser::Error) -> Self {
        Self::TomlSerialize(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(name: &str, target: Target, api_key: &str) -> Profile {
        Profile {
            name: name.to_string(),
            target,
            base_url: format!("https://{}.example.test", target),
            api_key: api_key.to_string(),
        }
    }

    #[test]
    fn add_rejects_duplicate_name_for_same_target() {
        let mut config = Config::default();
        config
            .add(profile("work", Target::Codex, "sk-first"))
            .expect("first profile should be added");

        let error = config
            .add(profile("work", Target::Codex, "sk-second"))
            .expect_err("duplicate target/name should fail");

        assert!(matches!(
            error,
            CapmError::DuplicateProfile {
                target: Target::Codex,
                name
            } if name == "work"
        ));
    }

    #[test]
    fn add_allows_same_name_for_different_targets() {
        let mut config = Config::default();
        config
            .add(profile("work", Target::Codex, "sk-codex"))
            .expect("codex profile should be added");
        config
            .add(profile("work", Target::Claude, "sk-claude"))
            .expect("claude profile should be added");

        assert_eq!(config.profiles.len(), 4);
    }

    #[test]
    fn switch_requires_target_for_ambiguous_name() {
        let mut config = Config {
            current: default_current_profiles(),
            profiles: vec![
                profile("work", Target::Codex, "sk-codex"),
                profile("work", Target::Claude, "sk-claude"),
            ],
        };

        let error = config
            .switch("work", None)
            .expect_err("ambiguous switch should fail");

        assert!(matches!(error, CapmError::AmbiguousProfile { name } if name == "work"));
    }

    #[test]
    fn switch_accepts_target_for_ambiguous_name() {
        let mut config = Config {
            current: default_current_profiles(),
            profiles: vec![
                profile("work", Target::Codex, "sk-codex"),
                profile("work", Target::Claude, "sk-claude"),
            ],
        };

        config
            .switch("work", Some(Target::Claude))
            .expect("target should disambiguate");

        assert_eq!(
            config.current_for_target(Target::Claude),
            Some(&CurrentProfile {
                target: Target::Claude,
                name: "work".to_string()
            })
        );
    }

    #[test]
    fn delete_falls_target_current_back_to_default_profile() {
        let mut config = Config {
            current: vec![CurrentProfile {
                target: Target::Codex,
                name: "work".to_string(),
            }],
            profiles: vec![profile("work", Target::Codex, "sk-codex")],
        };

        config
            .delete("work", None)
            .expect("active profile should be deleted");

        assert_eq!(
            config.current_for_target(Target::Codex),
            Some(&default_current_profile(Target::Codex))
        );
        assert!(
            config
                .profiles
                .iter()
                .any(|profile| { profile.name == "openai" && profile.target == Target::Codex })
        );
    }

    #[test]
    fn init_script_unsets_all_supported_variables_and_exports_current_profile() {
        let config = Config {
            current: vec![CurrentProfile {
                target: Target::Codex,
                name: "work".to_string(),
            }],
            profiles: vec![Profile {
                name: "work".to_string(),
                target: Target::Codex,
                base_url: "https://api.example.test/v1".to_string(),
                api_key: "sk-test'quote".to_string(),
            }],
        };

        let script = config.init_script(Shell::Zsh);

        assert!(script.contains("unset OPENAI_BASE_URL\n"));
        assert!(script.contains("unset ANTHROPIC_AUTH_TOKEN\n"));
        assert!(script.contains("export OPENAI_BASE_URL='https://api.example.test/v1'\n"));
        assert!(script.contains("export OPENAI_API_KEY='sk-test'\\''quote'\n"));
        assert!(!script.contains("export ANTHROPIC_AUTH_TOKEN"));
    }

    #[test]
    fn init_script_without_current_profile_only_unsets() {
        let script = Config {
            current: Vec::new(),
            profiles: default_profiles(),
        }
        .init_script(Shell::Zsh);

        assert!(script.contains("unset OPENAI_API_KEY\n"));
        assert!(!script.contains("export "));
    }

    #[test]
    fn default_config_includes_openai_and_anthropic_clear_profiles() {
        let config = Config::default();

        assert_eq!(
            config.current_for_target(Target::Codex),
            Some(&default_current_profile(Target::Codex))
        );
        assert_eq!(
            config.current_for_target(Target::Claude),
            Some(&default_current_profile(Target::Claude))
        );
        assert!(config.profiles.iter().any(|profile| {
            profile.name == "openai"
                && profile.target == Target::Codex
                && profile.base_url.is_empty()
                && profile.api_key.is_empty()
        }));
        assert!(config.profiles.iter().any(|profile| {
            profile.name == "anthropic"
                && profile.target == Target::Claude
                && profile.base_url.is_empty()
                && profile.api_key.is_empty()
        }));
    }

    #[test]
    fn switching_to_default_openai_profile_only_unsets_variables() {
        let mut config = Config::default();
        config
            .switch("openai", Some(Target::Codex))
            .expect("default openai profile should exist");

        let script = config.init_script(Shell::Zsh);

        assert!(script.contains("unset OPENAI_BASE_URL\n"));
        assert!(script.contains("unset OPENAI_API_KEY\n"));
        assert!(!script.contains("export "));
    }

    #[test]
    fn list_marks_default_current_profile_with_star() {
        let table = Config::default().render_table();

        assert!(table.contains("*"));
        assert!(table.contains("openai"));
    }

    #[test]
    fn mask_api_key_keeps_short_keys_fully_masked() {
        assert_eq!(mask_api_key(""), "");
        assert_eq!(mask_api_key("secret"), "******");
        assert_eq!(mask_api_key("123456789"), "1234*6789");
        assert_eq!(mask_api_key("123456789abcdef"), "1234*******cdef");
    }

    #[test]
    fn config_round_trips_toml() {
        let config = Config {
            current: vec![CurrentProfile {
                target: Target::Claude,
                name: "work".to_string(),
            }],
            profiles: vec![profile("work", Target::Claude, "sk-claude")],
        };

        let serialized = toml::to_string_pretty(&config).expect("config should serialize");
        let parsed: Config = toml::from_str(&serialized).expect("config should deserialize");

        assert_eq!(parsed, config);
    }

    #[test]
    fn config_path_uses_xdg_config_home_when_present() {
        let path = config_path_from(Some(PathBuf::from("/tmp/capm-test-config")))
            .expect("path should resolve");

        assert_eq!(
            path,
            PathBuf::from("/tmp/capm-test-config/capm/config.toml")
        );
    }

    #[cfg(unix)]
    #[test]
    fn save_writes_config_with_owner_only_permissions() {
        use std::os::unix::fs::PermissionsExt;
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("capm-test-{unique}"));
        let path = dir.join("config.toml");
        let config = Config {
            current: Vec::new(),
            profiles: vec![profile("work", Target::Codex, "sk-codex")],
        };

        config.save(&path).expect("config should save");

        let mode = fs::metadata(&path)
            .expect("config file should exist")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);

        fs::remove_dir_all(dir).expect("test dir should be removed");
    }
}
