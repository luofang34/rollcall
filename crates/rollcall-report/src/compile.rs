//! Writing the rendered LaTeX and compiling it with XeLaTeX against the
//! brand package.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::ReportError;

/// Writes the document as `reports/src/<number>-<date>.tex` and
/// returns the path.
pub fn write_tex_blocking(
    repo: &Path,
    number: &str,
    status_date: &str,
    tex: &str,
) -> Result<PathBuf, ReportError> {
    let src_dir = repo.join("reports").join("src");
    std::fs::create_dir_all(&src_dir).map_err(|source| ReportError::CreateDir {
        path: src_dir.clone(),
        source,
    })?;
    let path = src_dir.join(format!("{number}-{status_date}.tex"));
    std::fs::write(&path, tex).map_err(|source| ReportError::WriteTex {
        path: path.clone(),
        source,
    })?;
    Ok(path)
}

/// Compiles the document with XeLaTeX (two passes, so `lastpage` resolves),
/// copies the PDF into `reports/`, and returns the PDF path. The brand
/// submodule supplies `fleet-identity.sty` via `TEXINPUTS`.
pub fn compile_pdf_blocking(repo: &Path, tex_path: &Path) -> Result<PathBuf, ReportError> {
    let src_dir = tex_path.parent().unwrap_or(repo).to_path_buf();
    let brand = repo.join("brand");
    let texinputs = format!(
        "{}:{}:{}:",
        brand.display(),
        brand.join("assets").display(),
        brand.join("assets").join("logo").display()
    );
    for _pass in 0..2 {
        let output = Command::new("xelatex")
            .args(["-interaction=nonstopmode", "-halt-on-error"])
            .arg(format!("-output-directory={}", src_dir.display()))
            .arg(tex_path)
            .env("TEXINPUTS", &texinputs)
            .current_dir(repo)
            .output()
            .map_err(|source| ReportError::XelatexSpawn { source })?;
        if !output.status.success() {
            return Err(ReportError::CompileFailed {
                path: tex_path.to_path_buf(),
                log_tail: tail(&String::from_utf8_lossy(&output.stdout), 3000),
            });
        }
    }

    let pdf_src = tex_path.with_extension("pdf");
    let pdf_name = pdf_src.file_name().unwrap_or_default().to_owned();
    let out = repo.join("reports").join(pdf_name);
    std::fs::copy(&pdf_src, &out).map_err(|source| ReportError::CopyPdf {
        from: pdf_src,
        to: out.clone(),
        source,
    })?;
    Ok(out)
}

fn tail(text: &str, max_bytes: usize) -> String {
    let start = text.len().saturating_sub(max_bytes);
    let boundary = (start..=text.len())
        .find(|&i| text.is_char_boundary(i))
        .unwrap_or(0);
    text[boundary..].to_owned()
}
