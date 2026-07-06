//! Terminal output for subcommand results (tracing owns diagnostics).

use std::io::Write as _;

use tracing::warn;

/// Best-effort stdout write. The written files are the artifacts; a reader
/// that goes away (`rollcall status | head`) closes the pipe, and
/// println! would panic on it and abort the run mid-way.
pub fn print_to_stdout(text: &str) {
    let mut stdout = std::io::stdout().lock();
    if let Err(err) = stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.flush())
    {
        if err.kind() != std::io::ErrorKind::BrokenPipe {
            warn!("could not write to stdout: {err}");
        }
    }
}
