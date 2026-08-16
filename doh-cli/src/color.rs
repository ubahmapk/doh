use std::io::IsTerminal;

/// ANSI color scheme matching `q` (natesales/q): record name = purple, TTL
/// = green, record type = magenta, section labels = white. Bold intensity,
/// matching `q`'s `util.Color` (`\033[1;3Xm...\033[0m`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Name,
    Ttl,
    Type,
    Label,
}

impl Color {
    /// ANSI SGR codes, matching `q`'s `util.go` color map exactly:
    /// purple=34, green=32, magenta=35, white=37 (q's "purple" is ANSI
    /// code 34, conventionally "blue" — kept as-is to match q, not
    /// "corrected").
    fn code(self) -> &'static str {
        match self {
            Color::Name => "34",  // purple
            Color::Ttl => "32",   // green
            Color::Type => "35",  // magenta
            Color::Label => "37", // white
        }
    }
}

/// Whether color output is enabled for this run: explicit `--color`/
/// `--no-color` wins; otherwise `NO_COLOR` (if set to anything) disables
/// color; otherwise color is on iff stdout is a terminal. Order matches
/// `q`'s `main.go`: TTY-detect first, then `NO_COLOR` overrides to off.
pub fn resolve(color_flag: Option<bool>) -> bool {
    if let Some(explicit) = color_flag {
        return explicit;
    }
    let mut enabled = std::io::stdout().is_terminal();
    if std::env::var_os("NO_COLOR").is_some() {
        enabled = false;
    }
    enabled
}

/// Colorize `text` if `enabled`, otherwise return it unchanged.
pub fn paint(enabled: bool, color: Color, text: &str) -> String {
    if enabled {
        format!("\x1b[1;{}m{}\x1b[0m", color.code(), text)
    } else {
        text.to_string()
    }
}
