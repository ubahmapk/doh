use std::time::Duration;

use chrono::Local;
use doh_core::{Answer, DohError, OpCode, ParsedResponse};
use serde::Serialize;

use crate::color::{self, Color};
use crate::ttl::{self, TtlOpts};

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    Pretty,
    Column,
    Json,
    Yaml,
    Raw,
}

#[derive(Debug, Clone)]
pub struct DisplayOpts {
    pub format: Format,
    pub show_question: bool,
    pub show_answer: bool,
    pub show_authority: bool,
    pub show_additional: bool,
    pub show_stats: bool,
    pub short: bool,
    pub color: bool,
    pub ttl: TtlOpts,
}

/// The outcome of resolving one requested record type, paired with the
/// query metadata q's stats block needs (server label, elapsed time).
pub struct QueryResult {
    pub record_type: String,
    pub server: String,
    pub elapsed: Duration,
    pub outcome: Result<ParsedResponse, DohError>,
}

pub fn print(results: &[QueryResult], opts: &DisplayOpts) {
    match opts.format {
        Format::Pretty => print_pretty(results, opts),
        Format::Column => print_column(results, opts),
        Format::Raw => print_raw(results, opts),
        Format::Json | Format::Yaml => print_structured(results, opts),
    }
}

fn show_section_labels(opts: &DisplayOpts) -> bool {
    opts.show_question || opts.show_authority || opts.show_additional
}

fn sorted_by_type(records: &[Answer]) -> Vec<&Answer> {
    let mut sorted: Vec<&Answer> = records.iter().collect();
    sorted.sort_by_key(|a| a.record_type.to_string());
    sorted
}

fn print_pretty(results: &[QueryResult], opts: &DisplayOpts) {
    let mut first = true;
    for result in results {
        let response = match &result.outcome {
            Ok(r) => r,
            Err(e) => {
                eprintln!("error ({}): {e}", result.record_type);
                continue;
            }
        };

        let has_content = opts.show_question
            || (opts.show_answer && !response.answers.is_empty())
            || (opts.show_authority && !response.authorities.is_empty())
            || (opts.show_additional && !response.additionals.is_empty())
            || opts.show_stats;
        if !has_content {
            continue;
        }

        if !first {
            println!("\n\u{2500}\u{2500}\n");
        }
        first = false;

        if opts.show_question {
            println!("{}", color::paint(opts.color, Color::Label, "Question:"));
            println!(
                "{} {}",
                color::paint(opts.color, Color::Name, &response.question_name.to_string()),
                color::paint(opts.color, Color::Type, &response.question_type.to_string()),
            );
        }

        if opts.show_answer && !response.answers.is_empty() {
            if show_section_labels(opts) {
                println!("{}", color::paint(opts.color, Color::Label, "Answer:"));
            }
            print_section(&response.answers, opts, false);
        }
        if opts.show_authority && !response.authorities.is_empty() {
            println!("{}", color::paint(opts.color, Color::Label, "Authority:"));
            print_section(&response.authorities, opts, false);
        }
        if opts.show_additional && !response.additionals.is_empty() {
            println!("{}", color::paint(opts.color, Color::Label, "Additional:"));
            print_section(&response.additionals, opts, false);
        }

        if opts.show_stats {
            print_stats_pretty(response, result, opts);
        }
    }
}

fn print_section(records: &[Answer], opts: &DisplayOpts, column: bool) {
    let sorted = sorted_by_type(records);

    if opts.short {
        for a in &sorted {
            println!("{}", a.rdata);
        }
        return;
    }

    let longest_ttl = sorted
        .iter()
        .map(|a| ttl::format(a.ttl, opts.ttl).len())
        .max()
        .unwrap_or(0);
    let longest_type = sorted
        .iter()
        .map(|a| a.record_type.to_string().len())
        .max()
        .unwrap_or(0);

    for a in &sorted {
        let ttl_s = ttl::format(a.ttl, opts.ttl);
        let type_s = a.record_type.to_string();
        if column {
            println!(
                "{:>tw$} {:<lw$} {}",
                color::paint(opts.color, Color::Type, &type_s),
                color::paint(opts.color, Color::Ttl, &ttl_s),
                a.rdata,
                tw = longest_type,
                lw = longest_ttl,
            );
        } else {
            println!(
                "{} {} {} {}",
                color::paint(opts.color, Color::Name, &a.name.to_string()),
                color::paint(opts.color, Color::Ttl, &ttl_s),
                color::paint(opts.color, Color::Type, &type_s),
                a.rdata,
            );
        }
    }
}

fn print_column(results: &[QueryResult], opts: &DisplayOpts) {
    for result in results {
        match &result.outcome {
            Ok(response) if !response.answers.is_empty() => {
                print_section(&response.answers, opts, true);
            }
            Ok(_) => {}
            Err(e) => eprintln!("error ({}): {e}", result.record_type),
        }
    }
}

fn flags_string(r: &ParsedResponse) -> String {
    let mut flags = Vec::new();
    flags.push("qr"); // we only ever look at responses
    if r.authoritative {
        flags.push("aa");
    }
    if r.truncated {
        flags.push("tc");
    }
    if r.recursion_desired {
        flags.push("rd");
    }
    if r.recursion_available {
        flags.push("ra");
    }
    if r.authentic_data {
        flags.push("ad");
    }
    if r.checking_disabled {
        flags.push("cd");
    }
    flags.join(" ")
}

fn opcode_str(op: OpCode) -> &'static str {
    match op {
        OpCode::Query => "QUERY",
        OpCode::Status => "STATUS",
        OpCode::Notify => "NOTIFY",
        OpCode::Update => "UPDATE",
        _ => "UNKNOWN",
    }
}

fn print_stats_pretty(response: &ParsedResponse, result: &QueryResult, opts: &DisplayOpts) {
    println!("{}", color::paint(opts.color, Color::Label, "Stats:"));
    println!(
        "Received {} from {} in {} ({})",
        color::paint(
            opts.color,
            Color::Name,
            &format!("{} B", response.wire_size)
        ),
        color::paint(opts.color, Color::Ttl, &result.server),
        color::paint(opts.color, Color::Type, &format!("{:?}", result.elapsed)),
        color::paint(
            opts.color,
            Color::Type,
            &Local::now().format("%H:%M:%S %m-%d-%Y %Z").to_string()
        ),
    );
    println!(
        "Opcode: {} Status: {} ID {}: Flags: {} ({} Q {} A {} N {} E)",
        color::paint(opts.color, Color::Type, opcode_str(response.op_code)),
        color::paint(opts.color, Color::Ttl, &response.response_code.to_string()),
        color::paint(opts.color, Color::Ttl, &response.id.to_string()),
        color::paint(opts.color, Color::Name, &flags_string(response)),
        color::paint(opts.color, Color::Name, "1"),
        color::paint(opts.color, Color::Ttl, &response.answers.len().to_string()),
        color::paint(
            opts.color,
            Color::Type,
            &response.authorities.len().to_string()
        ),
        color::paint(
            opts.color,
            Color::Type,
            &response.additionals.len().to_string()
        ),
    );
}

fn print_raw(results: &[QueryResult], opts: &DisplayOpts) {
    let multi = results.len() > 1;
    for (i, result) in results.iter().enumerate() {
        let response = match &result.outcome {
            Ok(r) => r,
            Err(e) => {
                eprintln!(";; error ({}): {e}", result.record_type);
                continue;
            }
        };

        println!(
            ";; opcode: {}, status: {}, id: {}",
            opcode_str(response.op_code),
            response.response_code,
            response.id
        );
        println!(
            ";; flags: {}; QUERY: 1, ANSWER: {}, AUTHORITY: {}, ADDITIONAL: {}",
            flags_string(response),
            response.answers.len(),
            response.authorities.len(),
            response.additionals.len(),
        );

        println!("\n;; QUESTION SECTION:");
        println!(";{} IN {}", response.question_name, response.question_type);

        if !response.answers.is_empty() {
            println!("\n;; ANSWER SECTION:");
            for a in &response.answers {
                println!("{} {} IN {} {}", a.name, a.ttl, a.record_type, a.rdata);
            }
        }
        if opts.show_authority && !response.authorities.is_empty() {
            println!("\n;; AUTHORITY SECTION:");
            for a in &response.authorities {
                println!("{} {} IN {} {}", a.name, a.ttl, a.record_type, a.rdata);
            }
        }
        if opts.show_additional && !response.additionals.is_empty() {
            println!("\n;; ADDITIONAL SECTION:");
            for a in &response.additionals {
                println!("{} {} IN {} {}", a.name, a.ttl, a.record_type, a.rdata);
            }
        }

        if opts.show_stats {
            println!("\n;; Received {} B", response.wire_size);
            println!(";; Time {}", Local::now().format("%H:%M:%S %m-%d-%Y %Z"));
            println!(";; From {} in {:?}", result.server, result.elapsed);
        }

        if multi && i != results.len() - 1 {
            println!("\n--\n");
        }
    }
}

#[derive(Serialize)]
struct JsonRecord {
    name: String,
    ttl: u32,
    #[serde(rename = "type")]
    record_type: String,
    rdata: String,
}

impl From<&Answer> for JsonRecord {
    fn from(a: &Answer) -> Self {
        JsonRecord {
            name: a.name.to_string(),
            ttl: a.ttl,
            record_type: a.record_type.to_string(),
            rdata: a.rdata.to_string(),
        }
    }
}

#[derive(Serialize)]
struct JsonStats {
    wire_size: usize,
    server: String,
    elapsed_ms: u128,
    opcode: String,
    status: String,
    id: u16,
    flags: String,
}

#[derive(Serialize)]
struct JsonQuestion {
    name: String,
    #[serde(rename = "type")]
    record_type: String,
}

#[derive(Serialize)]
struct JsonReply {
    server: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    question: Option<JsonQuestion>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    answer: Vec<JsonRecord>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    authority: Vec<JsonRecord>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    additional: Vec<JsonRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stats: Option<JsonStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn print_structured(results: &[QueryResult], opts: &DisplayOpts) {
    let replies: Vec<JsonReply> = results
        .iter()
        .map(|result| match &result.outcome {
            Ok(response) => JsonReply {
                server: result.server.clone(),
                question: opts.show_question.then(|| JsonQuestion {
                    name: response.question_name.to_string(),
                    record_type: response.question_type.to_string(),
                }),
                answer: if opts.show_answer {
                    response.answers.iter().map(JsonRecord::from).collect()
                } else {
                    Vec::new()
                },
                authority: if opts.show_authority {
                    response.authorities.iter().map(JsonRecord::from).collect()
                } else {
                    Vec::new()
                },
                additional: if opts.show_additional {
                    response.additionals.iter().map(JsonRecord::from).collect()
                } else {
                    Vec::new()
                },
                stats: opts.show_stats.then(|| JsonStats {
                    wire_size: response.wire_size,
                    server: result.server.clone(),
                    elapsed_ms: result.elapsed.as_millis(),
                    opcode: opcode_str(response.op_code).to_string(),
                    status: response.response_code.to_string(),
                    id: response.id,
                    flags: flags_string(response),
                }),
                error: None,
            },
            Err(e) => JsonReply {
                server: result.server.clone(),
                question: None,
                answer: Vec::new(),
                authority: Vec::new(),
                additional: Vec::new(),
                stats: None,
                error: Some(e.to_string()),
            },
        })
        .collect();

    match opts.format {
        Format::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&replies).unwrap_or_default()
            );
        }
        Format::Yaml => {
            print!("{}", serde_norway::to_string(&replies).unwrap_or_default());
        }
        _ => unreachable!("print_structured only called for Json/Yaml"),
    }
}

/// True if this reply's response code should count as a failure for the
/// process exit code, matching `main`'s "query every requested type, exit
/// non-zero if any failed" policy.
pub fn is_err(result: &QueryResult) -> bool {
    result.outcome.is_err()
}
