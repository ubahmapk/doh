use std::process::ExitCode;
use std::str::FromStr;

use clap::Parser;
use doh_core::{DohTransport, HttpMethod, RecordType, Transport};

/// Resolve a DNS name over HTTPS (RFC 8484). No fallback to classic
/// UDP/TCP DNS: on failure this prints a clear error and exits non-zero.
#[derive(Parser)]
#[command(name = "doh", version, about)]
struct Args {
    /// Name to resolve, e.g. example.com
    name: String,

    /// DNS record type, e.g. A, AAAA, CNAME, TXT, MX
    #[arg(default_value = "A")]
    record_type: String,

    /// DoH server URL, e.g. https://dns.google/dns-query
    #[arg(short, long)]
    server: String,

    /// HTTP method used to send the query
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

    let transport = match DohTransport::new(&args.server, args.method.into()) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    match transport.resolve(&args.name, record_type).await {
        Ok(answers) if answers.is_empty() => {
            println!("{} has no {} records", args.name, args.record_type);
            ExitCode::SUCCESS
        }
        Ok(answers) => {
            for answer in answers {
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
