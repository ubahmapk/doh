use std::process::ExitCode;
use std::str::FromStr;

use clap::Parser;
use doh_core::{
    DohTransport, DoqTransport, DotTransport, HttpMethod, RecordType, ResponseCode, Transport,
};

/// Resolve a DNS name over a secure transport (DoH, DoT, or DoQ). No
/// fallback to classic UDP/TCP DNS: on failure this prints a clear error
/// and exits non-zero.
#[derive(Parser)]
#[command(name = "doh", version, about)]
struct Args {
    /// Name to resolve, e.g. example.com
    name: String,

    /// DNS record type, e.g. A, AAAA, CNAME, TXT, MX
    #[arg(default_value = "A")]
    record_type: String,

    /// Server address. `https://host/path` selects DNS-over-HTTPS;
    /// `tls://host[:port]` selects DNS-over-TLS; `quic://host[:port]`
    /// selects DNS-over-QUIC (default port 853 for both DoT and DoQ).
    #[arg(short, long)]
    server: String,

    /// HTTP method used to send the query (DoH only; ignored for DoT/DoQ)
    #[arg(long, value_enum, default_value = "get")]
    method: Method,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum Method {
    Get,
    Post,
}

impl From<Method> for HttpMethod {
    fn from(m: Method) -> Self {
        match m {
            Method::Get => HttpMethod::Get,
            Method::Post => HttpMethod::Post,
        }
    }
}

fn build_transport(
    server: &str,
    method: HttpMethod,
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

    let record_type = match RecordType::from_str(&args.record_type.to_uppercase()) {
        Ok(rt) => rt,
        Err(_) => {
            eprintln!("error: unknown record type '{}'", args.record_type);
            return ExitCode::FAILURE;
        }
    };

    let transport = match build_transport(&args.server, args.method.into()) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    match transport.resolve(&args.name, record_type).await {
        Ok(response) if response.response_code == ResponseCode::NXDomain => {
            println!("{} does not exist (NXDOMAIN)", args.name);
            ExitCode::SUCCESS
        }
        Ok(response) if response.answers.is_empty() => {
            println!("{} has no {} records", args.name, args.record_type);
            ExitCode::SUCCESS
        }
        Ok(response) => {
            for answer in response.answers {
                println!(
                    "{}\t{}\t{}\t{}",
                    answer.name, answer.ttl, answer.record_type, answer.rdata
                );
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
