//! Render the branded infrastructure PDF report.
//!
//! Computed sections (status, devices, workloads, power, capex) derive from
//! the declared data files and the newest status snapshot; authored analysis
//! lives in `reports/editorial/*.tex` fragments with `@@key@@` placeholders,
//! so prose is data, not compiled code. The assembled LaTeX compiles with
//! XeLaTeX against a brand LaTeX package.

pub mod compile;
pub mod document;
pub mod editorial;
mod error;
pub mod inputs;
pub mod power;
pub mod sections;
pub mod tex;

pub use compile::{compile_pdf_blocking, write_tex_blocking};
pub use document::render_document_blocking;
pub use error::ReportError;
pub use inputs::{ReportInputs, load_inputs_blocking};
pub use power::{PowerModel, PowerRow, power_model};
