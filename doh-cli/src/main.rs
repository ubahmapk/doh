mod color;
mod output;
mod ttl;

use std::process::ExitCode;
use std::str::FromStr;
use std::time::Instant;

use clap::Parser;
use doh_core::{DohTransport, DoqTransport, DotTransport, RecordType, Transport};
use output::{DisplayOpts, Format, QueryResult};
use ttl::TtlOpts;

/// Default record types queried when none are given, matching q's
/// `--default-rr-types` default list.
const DEFAULT_RECORD_TYPES: &[&str] = &["A", "AAAA", "NS", "MX", "TXT", "CNAME"];

/// Resolve a DNS name over a secure transport (DoH, DoT, or DoQ). No
/// fallback to classic UDP/TCP DNS: on failure this prints a clear error
/// and exits non-zero. Output formatting and flags closely follow `q`
/// (natesales/q).
#[derive(Parser)]
#[command(name = "doh", version, about)]
struct Args {
    /// Name to resolve, e.g. example.com
    name: String,

    /// DNS record type(s) to query, e.g. A AAAA MX. Defaults to
    /// A, AAAA, NS, MX, TXT, CNAME if none are given.
    record_types: Vec<String>,

    /// Server address. `https://host/path` selects DNS-over-HTTPS;
    /// `tls://host[:port]` selects DNS-over-TLS; `quic://host[:port]`
    /// selects DNS-over-QUIC (default port 853 for both DoT and DoQ).
    #[arg(short, long)]
    server: String,

    /// HTTP method used to send the query (DoH only; ignored for DoT/DoQ)
    #[arg(long, value_enum, default_value = "get")]
    method: Method,

    /// Output format
    #[arg(short = 'f', long, value_enum, default_value = "pretty")]
    format: Format,

    /// Show question section
    #[arg(long, default_value_t = false)]
    question: bool,

    /// Show answer section
    #[arg(long, default_value_t = true)]
    answer: bool,

    /// Show authority section
    #[arg(long, default_value_t = false)]
    authority: bool,

    /// Show additional section
    #[arg(long, default_value_t = false)]
    additional: bool,

    /// Show all sections and statistics
    #[arg(long, default_value_t = false)]
    all: bool,

    /// Show time and message statistics
    #[arg(short = 'S', long, default_value_t = false)]
    stats: bool,

    /// Show record values only
    #[arg(short = 'r', long, default_value_t = false)]
    short: bool,

    /// Format TTLs in human readable form (e.g. 24h0m0s)
    #[arg(long, default_value_t = true)]
    pretty_ttls: bool,

    /// Remove zero components of pretty TTLs (24h0m0s -> 24h)
    #[arg(long, default_value_t = true)]
    short_ttls: bool,

    /// Round TTLs down to the nearest minute
    #[arg(long, default_value_t = false)]
    round_ttls: bool,

    /// Enable color output (default: auto-detect terminal, honors NO_COLOR)
    #[arg(long)]
    color: bool,

    /// Disable color output
    #[arg(long, conflicts_with = "color")]
    no_color: bool,
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

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();

    let requested_types: Vec<String> = if args.record_types.is_empty() {
        DEFAULT_RECORD_TYPES.iter().map(|s| s.to_string()).collect()
    } else {
        args.record_types.clone()
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

    let transport = match build_transport(&args.server, args.method.into()) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    let mut results = Vec::with_capacity(record_types.len());
    for record_type in record_types {
        let start = Instant::now();
        let outcome = transport.resolve(&args.name, record_type).await;
        results.push(QueryResult {
            record_type: record_type.to_string(),
            server: args.server.clone(),
            elapsed: start.elapsed(),
            outcome,
        });
    }

    let show_stats = args.stats || args.all;
    let opts = DisplayOpts {
        format: args.format,
        show_question: args.question || args.all,
        show_answer: args.answer,
        show_authority: args.authority || args.all,
        show_additional: args.additional || args.all,
        show_stats,
        short: args.short,
        color: color::resolve(if args.no_color {
            Some(false)
        } else if args.color {
            Some(true)
        } else {
            None
        }),
        ttl: TtlOpts {
            pretty: args.pretty_ttls,
            short: args.short_ttls,
            round: args.round_ttls,
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
