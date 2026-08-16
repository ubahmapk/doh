use std::time::Duration;

/// TTL display options, matching `q`'s flags of the same names/defaults.
#[derive(Debug, Clone, Copy)]
pub struct TtlOpts {
    pub pretty: bool,
    pub short: bool,
    pub round: bool,
}

impl Default for TtlOpts {
    fn default() -> Self {
        Self {
            pretty: true,
            short: true,
            round: false,
        }
    }
}

/// Round a TTL down to the nearest whole minute, matching q's
/// `ttl - (ttl % 60)` (`main.go`, `--round-ttls`).
pub fn round(ttl_secs: u32) -> u32 {
    ttl_secs - (ttl_secs % 60)
}

/// Format a TTL for display per `opts`, matching q's `parseRR`
/// (`output/pretty.go`) exactly:
/// - plain: the TTL in seconds as a bare number.
/// - pretty: Go's `time.Duration` string form (e.g. `24h0m0s`, `45s`).
/// - pretty + short: the same two substring replacements q performs
///   (`"m0s"` -> `"m"`, then `"h0m"` -> `"h"`) -- this is a literal port of
///   q's string-replace approach, including its quirks (e.g. `1h0m45s`
///   becomes `1h45s`), not a more "correct" reformatting.
pub fn format(ttl_secs: u32, opts: TtlOpts) -> String {
    let ttl_secs = if opts.round {
        round(ttl_secs)
    } else {
        ttl_secs
    };

    if !opts.pretty {
        return ttl_secs.to_string();
    }

    let mut s = format_go_duration(Duration::from_secs(ttl_secs as u64));
    if opts.short {
        s = s.replace("m0s", "m").replace("h0m", "h");
    }
    s
}

/// Replicate Go's `time.Duration.String()` for whole-second durations:
/// `[Nh][Nm]Ns`, omitting leading zero units but not trailing ones within
/// a unit that's present (e.g. `24h0m0s`, `1h30m0s`, `45s`, `0s`).
fn format_go_duration(d: Duration) -> String {
    let total_secs = d.as_secs();
    if total_secs == 0 {
        return "0s".to_string();
    }

    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{hours}h{minutes}m{seconds}s")
    } else if minutes > 0 {
        format!("{minutes}m{seconds}s")
    } else {
        format!("{seconds}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_ttl_is_bare_number() {
        let opts = TtlOpts {
            pretty: false,
            short: false,
            round: false,
        };
        assert_eq!(format(86400, opts), "86400");
    }

    #[test]
    fn pretty_ttl_matches_go_duration_string() {
        let opts = TtlOpts {
            pretty: true,
            short: false,
            round: false,
        };
        assert_eq!(format(86400, opts), "24h0m0s");
        assert_eq!(format(5400, opts), "1h30m0s");
        assert_eq!(format(45, opts), "45s");
        assert_eq!(format(0, opts), "0s");
    }

    #[test]
    fn short_ttl_matches_qs_example() {
        // q's own README/help text example: 24h0m0s -> 24h
        let opts = TtlOpts::default();
        assert_eq!(format(86400, opts), "24h");
        assert_eq!(format(5400, opts), "1h30m");
        assert_eq!(format(45, opts), "45s");
    }

    #[test]
    fn round_ttl_rounds_down_to_the_minute() {
        assert_eq!(round(125), 120);
        assert_eq!(round(60), 60);
        assert_eq!(round(59), 0);
    }
}
