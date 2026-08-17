use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Optional defaults loaded from a TOML config file. Every field is
/// optional so a config file may set any subset of these keys; missing
/// keys fall through to the CLI's own hardcoded defaults. CLI flags always
/// take precedence over config values (see `main.rs`'s `merged` helper).
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub server: Option<String>,
    pub method: Option<String>,
    pub default_record_types: Option<Vec<String>>,

    pub format: Option<String>,

    pub question: Option<bool>,
    pub answer: Option<bool>,
    pub authority: Option<bool>,
    pub additional: Option<bool>,
    pub all: Option<bool>,
    pub stats: Option<bool>,
    pub short: Option<bool>,

    pub pretty_ttls: Option<bool>,
    pub short_ttls: Option<bool>,
    pub round_ttls: Option<bool>,

    pub color: Option<bool>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("could not read config file {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not parse config file {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
}

/// The default config file path: `<OS config dir>/doh/config.toml` (e.g.
/// `~/.config/doh/config.toml` on Linux, `~/Library/Application
/// Support/doh/config.toml` on macOS, `%APPDATA%\doh\config.toml` on
/// Windows), or `None` if the OS's home/config directory can't be
/// determined.
pub fn default_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "doh").map(|dirs| dirs.config_dir().join("config.toml"))
}

/// Resolve the effective config path: `override_path` if given, else
/// [`default_path`].
pub fn resolve_path(override_path: Option<&Path>) -> Option<PathBuf> {
    override_path.map(Path::to_path_buf).or_else(default_path)
}

/// Load config from `path`. A missing file is not an error -- it just
/// means no config-file defaults are set, and this returns
/// `Config::default()`. A present-but-malformed file *is* an error: never
/// silently ignored.
pub fn load(path: &Path) -> Result<Config, ConfigError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(source) => {
            return Err(ConfigError::Io {
                path: path.to_path_buf(),
                source,
            })
        }
    };

    toml::from_str(&text).map_err(|source| ConfigError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_yields_default_config() {
        let path = std::env::temp_dir().join("doh-config-test-missing-does-not-exist.toml");
        let config = load(&path).expect("missing file is not an error");
        assert!(config.server.is_none());
        assert!(config.answer.is_none());
    }

    #[test]
    fn malformed_toml_is_a_clear_error() {
        let path = std::env::temp_dir().join("doh-config-test-malformed.toml");
        std::fs::write(&path, "this is not valid = = toml").unwrap();
        let err = load(&path).unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn partial_config_leaves_other_fields_none() {
        let path = std::env::temp_dir().join("doh-config-test-partial.toml");
        std::fs::write(
            &path,
            "server = \"https://dns.google/dns-query\"\nstats = true\n",
        )
        .unwrap();
        let config = load(&path).expect("valid partial config");
        assert_eq!(
            config.server.as_deref(),
            Some("https://dns.google/dns-query")
        );
        assert_eq!(config.stats, Some(true));
        assert!(config.answer.is_none());
        assert!(config.format.is_none());
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn resolve_path_prefers_override() {
        let override_path = Path::new("/tmp/custom-doh-config.toml");
        assert_eq!(
            resolve_path(Some(override_path)),
            Some(override_path.to_path_buf())
        );
    }
}
