mod color;
mod config;
mod output;
mod ttl;

use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;
use std::time::Instant;

use clap::Parser;
use doh_core::{DohTransport, DoqTransport, DotTransport, RecordType, Transport};
use output::{DisplayOpts, Format, QueryResult};
use ttl::TtlOpts;

/// Default record types queried when none are given (CLI or config),
/// matching q's `--default-rr-types` default list.
const DEFAULT_RECORD_TYPES: &[&str] = &["A", "AAAA", "NS", "MX", "TXT", "CNAME"];

/// Resolve a DNS name over a secure transport (DoH, DoT, or DoQ). No
/// fallback to classic UDP/TCP DNS: on failure this prints a clear error
/// and exits non-zero. Output formatting and flags closely follow `q`
/// (natesales/q). Defaults for most flags can be set in a config file;
/// see `--config`.
#[derive(Parser)]
#[command(name = "doh", version, about)]
struct Args {
    /// Name to resolve, e.g. example.com
    #[arg(required_unless_present = "init_config")]
    name: Option<String>,

    /// DNS record type(s) to query, e.g. A AAAA MX. Defaults to
    /// `default_record_types` from the config file if set, else
    /// A, AAAA, NS, MX, TXT, CNAME.
    record_types: Vec<String>,

    /// Server address. `https://host/path` selects DNS-over-HTTPS;
    /// `tls://host[:port]` selects DNS-over-TLS; `quic://host[:port]`
    /// selects DNS-over-QUIC (default port 853 for both DoT and DoQ).
    /// Falls back to `server` in the config file if not given.
    #[arg(short, long)]
    server: Option<String>,

    /// Path to the config file (default: OS-specific config dir, e.g.
    /// ~/.config/doh/config.toml on Linux)
    #[arg(long)]
    config: Option<PathBuf>,

    /// Create a fully commented-out config template at the config path
    /// (see --config) and exit. Fails if a file is already there.
    #[arg(long)]
    init_config: bool,

    /// HTTP method used to send the query (DoH only; ignored for DoT/DoQ)
    #[arg(long, value_enum)]
    method: Option<Method>,

    /// Output format
    #[arg(short = 'f', long, value_enum)]
    format: Option<Format>,

    /// Show question section
    #[arg(long)]
    question: bool,
    /// Don't show question section (overrides a config default of true)
    #[arg(long = "no-question", conflicts_with = "question")]
    no_question: bool,

    /// Show answer section (default: on)
    #[arg(long)]
    answer: bool,
    /// Don't show answer section (overrides a config default of true)
    #[arg(long = "no-answer", conflicts_with = "answer")]
    no_answer: bool,

    /// Show authority section
    #[arg(long)]
    authority: bool,
    /// Don't show authority section (overrides a config default of true)
    #[arg(long = "no-authority", conflicts_with = "authority")]
    no_authority: bool,

    /// Show additional section
    #[arg(long)]
    additional: bool,
    /// Don't show additional section (overrides a config default of true)
    #[arg(long = "no-additional", conflicts_with = "additional")]
    no_additional: bool,

    /// Show all sections and statistics
    #[arg(long)]
    all: bool,
    /// Don't show all sections/statistics (overrides a config default of true)
    #[arg(long = "no-all", conflicts_with = "all")]
    no_all: bool,

    /// Show time and message statistics
    #[arg(short = 'S', long)]
    stats: bool,
    /// Don't show statistics (overrides a config default of true)
    #[arg(long = "no-stats", conflicts_with = "stats")]
    no_stats: bool,

    /// Show record values only
    #[arg(short = 'r', long)]
    short: bool,
    /// Show full name/ttl/type/value columns (overrides a config default of true)
    #[arg(long = "no-short", conflicts_with = "short")]
    no_short: bool,

    /// Format TTLs in human readable form (e.g. 24h0m0s) (default: on)
    #[arg(long)]
    pretty_ttls: bool,
    /// Show TTLs as plain seconds (overrides a config default of true)
    #[arg(long = "no-pretty-ttls", conflicts_with = "pretty_ttls")]
    no_pretty_ttls: bool,

    /// Remove zero components of pretty TTLs (24h0m0s -> 24h) (default: on)
    #[arg(long)]
    short_ttls: bool,
    /// Keep zero components of pretty TTLs (overrides a config default of true)
    #[arg(long = "no-short-ttls", conflicts_with = "short_ttls")]
    no_short_ttls: bool,

    /// Round TTLs down to the nearest minute
    #[arg(long)]
    round_ttls: bool,
    /// Don't round TTLs (overrides a config default of true)
    #[arg(long = "no-round-ttls", conflicts_with = "round_ttls")]
    no_round_ttls: bool,

    /// Enable color output (default: auto-detect terminal, honors NO_COLOR)
    #[arg(long)]
    color: bool,

    /// Disable color output
    #[arg(long, conflicts_with = "color")]
    no_color: bool,
}

/// Resolve a tri-state boolean flag pair (`--x`/`--no-x`) to `Some(true)`,
/// `Some(false)`, or `None` if neither was passed on the CLI.
fn tri_state(positive: bool, negative: bool) -> Option<bool> {
    if positive {
        Some(true)
    } else if negative {
        Some(false)
    } else {
        None
    }
}

/// CLI value, then config value, then the hardcoded default -- the
/// precedence order for every mergeable setting.
fn merged<T>(cli: Option<T>, config: Option<T>, default: T) -> T {
    cli.or(config).unwrap_or(default)
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum Method {
    Get,
    Post,
}

impl From<Method> for doh_core::HttpMethod {
    fn from(m: Method) -> Self {
        match m {
            Method::Get => doh_core::HttpMethod::Get,
            Method::Post => doh_core::HttpMethod::Post,
        }
    }
}

impl FromStr for Method {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "get" => Ok(Method::Get),
            "post" => Ok(Method::Post),
            other => Err(format!("unknown method '{other}' (expected get or post)")),
        }
    }
}

fn build_transport(
    server: &str,
    method: doh_core::HttpMethod,
) -> Result<Box<dyn Transport>, doh_core::DohError> {
    if let Some(addr) = server.strip_prefix("tls://") {
        Ok(Box::new(DotTransport::new(addr)?))
    } else if let Some(addr) = server.strip_prefix("quic://") {
        Ok(Box::new(DoqTransport::new(addr)?))
    } else {
        Ok(Box::new(DohTransport::new(server, method)?))
    }
}

/// Reset SIGPIPE to its default disposition. Rust ignores SIGPIPE by
/// default, so writing to a closed pipe (e.g. `doh ... | head`) surfaces
/// as an `Err` that `println!` panics on. Resetting to `SIG_DFL` restores
/// the usual Unix behavior: the OS kills the process silently, matching
/// tools like `dig`.
#[cfg(unix)]
fn reset_sigpipe() {
    // nosemgrep: rust.lang.security.unsafe-usage.unsafe-usage -- single, narrow libc::signal FFI call, no memory unsafety
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

#[cfg(not(unix))]
fn reset_sigpipe() {}

#[tokio::main]
async fn main() -> ExitCode {
    reset_sigpipe();

    let args = Args::parse();

    let config_path = config::resolve_path(args.config.as_deref());

    if args.init_config {
        let Some(path) = &config_path else {
            eprintln!("error: could not determine a config directory for this OS");
            return ExitCode::FAILURE;
        };
        return match config::init(path) {
            Ok(()) => {
                println!("Created config template at {}", path.display());
                ExitCode::SUCCESS
            }
            Err(e @ config::ConfigError::AlreadyExists { .. }) => {
                eprintln!("warning: {e}");
                ExitCode::FAILURE
            }
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        };
    }

    // Guaranteed `Some` here: clap's `required_unless_present = "init_config"`
    // means the only way to reach this point with `name` absent is the
    // `--init-config` branch above, which already returned.
    let name = args
        .name
        .clone()
        .expect("name is required unless --init-config");

    let cfg = match &config_path {
        Some(path) => match config::load(path) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::FAILURE;
            }
        },
        None => config::Config::default(),
    };

    let server = match args.server.clone().or_else(|| cfg.server.clone()) {
        Some(s) => s,
        None => {
            let hint = config_path
                .as_ref()
                .map(|p| format!(" and none set in {}", p.display()))
                .unwrap_or_default();
            eprintln!("error: no --server given{hint}");
            return ExitCode::FAILURE;
        }
    };

    let method: Method = match args.method {
        Some(m) => m,
        None => match &cfg.method {
            Some(s) => match Method::from_str(s) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("error: invalid 'method' in config: {e}");
                    return ExitCode::FAILURE;
                }
            },
            None => Method::Get,
        },
    };

    let requested_types: Vec<String> = if !args.record_types.is_empty() {
        args.record_types.clone()
    } else if let Some(types) = &cfg.default_record_types {
        types.clone()
    } else {
        DEFAULT_RECORD_TYPES.iter().map(|s| s.to_string()).collect()
    };

    let mut record_types = Vec::with_capacity(requested_types.len());
    for rt in &requested_types {
        match RecordType::from_str(&rt.to_uppercase()) {
            Ok(parsed) => record_types.push(parsed),
            Err(_) => {
                eprintln!("error: unknown record type '{rt}'");
                return ExitCode::FAILURE;
            }
        }
    }

    let transport = match build_transport(&server, method.into()) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    let mut results = Vec::with_capacity(record_types.len());
    for record_type in record_types {
        let start = Instant::now();
        let outcome = transport.resolve(&name, record_type).await;
        results.push(QueryResult {
            record_type: record_type.to_string(),
            server: server.clone(),
            elapsed: start.elapsed(),
            outcome,
        });
    }

    let config_format = match &cfg.format {
        Some(s) => match parse_format(s) {
            Some(f) => Some(f),
            None => {
                eprintln!("error: invalid 'format' in config: '{s}'");
                return ExitCode::FAILURE;
            }
        },
        None => None,
    };
    let format = merged(args.format, config_format, Format::Pretty);

    let show_question = merged(
        tri_state(args.question, args.no_question),
        cfg.question,
        false,
    );
    let show_answer = merged(tri_state(args.answer, args.no_answer), cfg.answer, true);
    let show_authority = merged(
        tri_state(args.authority, args.no_authority),
        cfg.authority,
        false,
    );
    let show_additional = merged(
        tri_state(args.additional, args.no_additional),
        cfg.additional,
        false,
    );
    let all = merged(tri_state(args.all, args.no_all), cfg.all, false);
    let stats = merged(tri_state(args.stats, args.no_stats), cfg.stats, false);
    let short = merged(tri_state(args.short, args.no_short), cfg.short, false);
    let pretty_ttls = merged(
        tri_state(args.pretty_ttls, args.no_pretty_ttls),
        cfg.pretty_ttls,
        true,
    );
    let short_ttls = merged(
        tri_state(args.short_ttls, args.no_short_ttls),
        cfg.short_ttls,
        true,
    );
    let round_ttls = merged(
        tri_state(args.round_ttls, args.no_round_ttls),
        cfg.round_ttls,
        false,
    );

    let cli_color = tri_state(args.color, args.no_color);
    let color = color::resolve(cli_color.or(cfg.color));

    let opts = DisplayOpts {
        format,
        show_question: show_question || all,
        show_answer,
        show_authority: show_authority || all,
        show_additional: show_additional || all,
        show_stats: stats || all,
        short,
        color,
        ttl: TtlOpts {
            pretty: pretty_ttls,
            short: short_ttls,
            round: round_ttls,
        },
    };

    let any_error = results.iter().any(output::is_err);

    output::print(&results, &opts);

    if any_error {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn parse_format(s: &str) -> Option<Format> {
    match s.to_ascii_lowercase().as_str() {
        "pretty" => Some(Format::Pretty),
        "column" => Some(Format::Column),
        "json" => Some(Format::Json),
        "yaml" => Some(Format::Yaml),
        "raw" => Some(Format::Raw),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // tri_state: precedence bugs here are silent -- a config value failing
    // to override, or overriding when it shouldn't, wouldn't error or
    // crash, it would just quietly do the wrong thing. Worth pinning down
    // exactly.

    #[test]
    fn tri_state_positive_flag_is_some_true() {
        assert_eq!(tri_state(true, false), Some(true));
    }

    #[test]
    fn tri_state_negative_flag_is_some_false() {
        assert_eq!(tri_state(false, true), Some(false));
    }

    #[test]
    fn tri_state_neither_flag_is_none() {
        assert_eq!(tri_state(false, false), None);
    }

    #[test]
    fn tri_state_positive_wins_if_somehow_both_set() {
        // clap's `conflicts_with` prevents `--x --no-x` together at the CLI
        // layer, but the function itself doesn't enforce that -- document
        // its actual (positive-wins) behavior directly.
        assert_eq!(tri_state(true, true), Some(true));
    }

    // merged: CLI > config > hardcoded default, for every mergeable
    // setting (server, method, format, every section/TTL/color flag).

    #[test]
    fn merged_cli_value_wins_over_config_and_default() {
        assert_eq!(merged(Some(1), Some(2), 3), 1);
    }

    #[test]
    fn merged_config_value_wins_over_default_when_cli_absent() {
        assert_eq!(merged(None, Some(2), 3), 2);
    }

    #[test]
    fn merged_default_used_when_cli_and_config_absent() {
        assert_eq!(merged(None::<i32>, None, 3), 3);
    }

    #[test]
    fn merged_works_with_bool_and_string_types_too() {
        assert!(merged(Some(true), None, false));
        assert_eq!(
            merged(None, Some("config".to_string()), "default".to_string()),
            "config"
        );
    }
}
