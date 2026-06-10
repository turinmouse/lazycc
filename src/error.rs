use std::fmt;
use std::io;

use crate::config::Target;

#[derive(Debug)]
pub(crate) enum LazyccError {
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

impl fmt::Display for LazyccError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LazyccError::AmbiguousProfile { name } => {
                write!(
                    f,
                    "profile '{name}' exists for multiple targets; pass --target"
                )
            }
            LazyccError::ConfigDirUnavailable => {
                write!(f, "could not determine user config directory")
            }
            LazyccError::DuplicateProfile { target, name } => {
                write!(f, "profile '{name}' already exists for target '{target}'")
            }
            LazyccError::Io(error) => write!(f, "{error}"),
            LazyccError::ProfileNotFound { target, name } => match target {
                Some(target) => write!(f, "profile '{name}' for target '{target}' was not found"),
                None => write!(f, "profile '{name}' was not found"),
            },
            LazyccError::Prompt(error) => write!(f, "{error}"),
            LazyccError::TomlDeserialize(error) => write!(f, "{error}"),
            LazyccError::TomlSerialize(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for LazyccError {}

impl From<io::Error> for LazyccError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<inquire::error::InquireError> for LazyccError {
    fn from(error: inquire::error::InquireError) -> Self {
        Self::Prompt(error)
    }
}

impl From<toml::de::Error> for LazyccError {
    fn from(error: toml::de::Error) -> Self {
        Self::TomlDeserialize(error)
    }
}

impl From<toml::ser::Error> for LazyccError {
    fn from(error: toml::ser::Error) -> Self {
        Self::TomlSerialize(error)
    }
}
