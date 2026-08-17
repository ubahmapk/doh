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
    #[error("config file already exists at {path}; not overwriting")]
    AlreadyExists { path: PathBuf },
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

/// A fully commented-out, documented config template. Every key is
/// present but commented out, so writing this file as-is (nothing
/// uncommented) changes no behavior -- it's the same as having no config
/// file at all. Keep this in sync with the README's "Configuration"
/// section by hand; both describe the same small, static key set.
const TEMPLATE: &str = r#"# doh config file
#
# Every key below is optional and commented out. Uncomment and edit the
# ones you want to set. Precedence: CLI flag > this file > built-in
# default. Run `doh --help` for the CLI flag each key corresponds to.

# --- Connection ---

# Default server, used when --server isn't given on the CLI.
# server = "https://dns.google/dns-query"

# HTTP method for DoH queries: "get" or "post" (default: get)
# method = "get"

# Record types queried when none are given positionally
# (default: ["A", "AAAA", "NS", "MX", "TXT", "CNAME"])
# default_record_types = ["A", "AAAA"]

# --- Output ---

# "pretty" | "column" | "json" | "yaml" | "raw" (default: pretty)
# format = "pretty"

# --- Sections ---

# question = false
# answer = true
# authority = false
# additional = false
# all = false
# stats = false
# short = false

# --- TTL display ---

# pretty_ttls = true
# short_ttls = true
# round_ttls = false

# --- Color ---

# on if stdout is a terminal, off when piped/NO_COLOR is set, unless set here
# color = true
"#;

/// Write a fresh, fully commented-out config template to `path`. Fails
/// with [`ConfigError::AlreadyExists`] rather than overwriting if a file
/// is already there; creates the parent directory first if needed (the
/// `directories` crate only computes the path, it doesn't create it).
pub fn init(path: &Path) -> Result<(), ConfigError> {
    if path.exists() {
        return Err(ConfigError::AlreadyExists {
            path: path.to_path_buf(),
        });
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| ConfigError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    }

    std::fs::write(path, TEMPLATE).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_yields_default_config() {
        // nosemgrep: rust.lang.security.temp-dir.temp-dir -- test-only scratch file, no privilege boundary
        let path = std::env::temp_dir().join("doh-config-test-missing-does-not-exist.toml");
        let config = load(&path).expect("missing file is not an error");
        assert!(config.server.is_none());
        assert!(config.answer.is_none());
    }

    #[test]
    fn malformed_toml_is_a_clear_error() {
        // nosemgrep: rust.lang.security.temp-dir.temp-dir -- test-only scratch file, no privilege boundary
        let path = std::env::temp_dir().join("doh-config-test-malformed.toml");
        std::fs::write(&path, "this is not valid = = toml").unwrap();
        let err = load(&path).unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }));
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn partial_config_leaves_other_fields_none() {
        // nosemgrep: rust.lang.security.temp-dir.temp-dir -- test-only scratch file, no privilege boundary
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

    #[test]
    fn init_writes_a_template_that_round_trips_as_all_none() {
        // nosemgrep: rust.lang.security.temp-dir.temp-dir -- test-only scratch dir, no privilege boundary
        let dir = std::env::temp_dir().join(format!("doh-config-init-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");

        init(&path).expect("fresh path can be initialized");
        let config = load(&path).expect("template is valid TOML");

        // Every key is commented out, so nothing should be set -- the
        // template is behavior-inert as-is.
        assert!(config.server.is_none());
        assert!(config.method.is_none());
        assert!(config.default_record_types.is_none());
        assert!(config.format.is_none());
        assert!(config.question.is_none());
        assert!(config.answer.is_none());
        assert!(config.authority.is_none());
        assert!(config.additional.is_none());
        assert!(config.all.is_none());
        assert!(config.stats.is_none());
        assert!(config.short.is_none());
        assert!(config.pretty_ttls.is_none());
        assert!(config.short_ttls.is_none());
        assert!(config.round_ttls.is_none());
        assert!(config.color.is_none());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn init_does_not_overwrite_an_existing_file() {
        // nosemgrep: rust.lang.security.temp-dir.temp-dir -- test-only scratch dir, no privilege boundary
        let dir = std::env::temp_dir().join(format!(
            "doh-config-init-exists-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "server = \"https://existing.example/dns-query\"\n").unwrap();

        let err = init(&path).unwrap_err();
        assert!(matches!(err, ConfigError::AlreadyExists { .. }));

        // Untouched, byte-for-byte -- no partial overwrite.
        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            contents,
            "server = \"https://existing.example/dns-query\"\n"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn init_creates_missing_parent_directories() {
        // nosemgrep: rust.lang.security.temp-dir.temp-dir -- test-only scratch dir, no privilege boundary
        let dir = std::env::temp_dir().join(format!(
            "doh-config-init-mkdir-test-{}/nested/dirs",
            std::process::id()
        ));
        let path = dir.join("config.toml");
        assert!(!dir.exists());

        init(&path).expect("parent directories are created automatically");
        assert!(path.exists());

        std::fs::remove_dir_all(dir.parent().unwrap().parent().unwrap()).ok();
    }
}
