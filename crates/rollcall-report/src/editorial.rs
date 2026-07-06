//! Editorial prose fragments: committed LaTeX files under
//! `reports/editorial/` with `@@key@@` placeholders for computed values.
//! Authored analysis lives there as data, so a report cycle edits fragments,
//! not compiled code.

use std::collections::BTreeMap;
use std::path::Path;

use crate::error::ReportError;

/// Loads a fragment and substitutes every `@@key@@` placeholder from
/// `values`. Unknown keys and unterminated markers are errors — a silently
/// skipped placeholder would ship a report with a hole in it.
pub fn render_fragment_blocking(
    dir: &Path,
    name: &str,
    values: &BTreeMap<String, String>,
) -> Result<String, ReportError> {
    let path = dir.join(name);
    let text = std::fs::read_to_string(&path).map_err(|source| ReportError::ReadFragment {
        path: path.clone(),
        source,
    })?;
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some(start) = rest.find("@@") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("@@") else {
            return Err(ReportError::UnterminatedPlaceholder { path });
        };
        let key = &after[..end];
        match values.get(key) {
            Some(value) => out.push_str(value),
            None => {
                return Err(ReportError::UnknownPlaceholder {
                    path,
                    key: key.to_owned(),
                });
            }
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    Ok(out)
}

#[cfg(test)]
mod tests;
