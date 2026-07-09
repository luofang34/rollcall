//! LaTeX-side primitives: escaping, badges, and Python-format-compatible
//! number rendering (the committed reports were produced with `:,.Nf`
//! formatting; these helpers keep the output stable).

use rollcall_status::ProbeState;

/// Escapes LaTeX-special characters in data-derived text, and maps the few
/// Unicode symbols that recur in probe/device prose but have no glyph in the
/// report font (they would render as a tofu box). Backslash is escaped first
/// so escapes this introduces are not re-escaped; the symbol maps run last so
/// the `$` and `\` they insert are left intact.
pub fn esc(s: &str) -> String {
    let mut out = s.to_owned();
    for (from, to) in [
        ("\\", r"\textbackslash{}"),
        ("&", r"\&"),
        ("%", r"\%"),
        ("#", r"\#"),
        ("_", r"\_"),
        ("$", r"\$"),
        ("~", r"\~{}"),
        ("→", r"$\rightarrow$"),
        ("←", r"$\leftarrow$"),
        ("↔", r"$\leftrightarrow$"),
        ("≈", r"$\approx$"),
    ] {
        out = out.replace(from, to);
    }
    out
}

/// The colored state marker used in status tables.
pub fn state_badge(state: ProbeState) -> &'static str {
    match state {
        ProbeState::Up => r"\textcolor{success}{\textbf{UP}}",
        ProbeState::Down => r"\textcolor{accent}{\textbf{DOWN}}",
        ProbeState::Unverifiable => r"\textcolor{darkgray}{\textbf{UNVERIFIABLE}}",
    }
}

/// Formats with `decimals` fraction digits and comma thousands separators,
/// like Python's `:,.Nf` (`1850.4` at 0 decimals → `1,850`).
pub fn fmt_sep(value: f64, decimals: usize) -> String {
    insert_separators(&format!("{value:.decimals$}"))
}

/// Comma thousands separators for a non-negative integer (Python `:,`).
pub fn fmt_int_sep(value: i64) -> String {
    insert_separators(&value.to_string())
}

fn insert_separators(plain: &str) -> String {
    let (int_part, frac) = match plain.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (plain, None),
    };
    let digits: Vec<char> = int_part.chars().collect();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.iter().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(*c);
    }
    match frac {
        Some(f) => format!("{grouped}.{f}"),
        None => grouped,
    }
}

#[cfg(test)]
mod tests;
