//! Error type for the core. Messages are plain sentences so the interface can
//! show them as they are.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not read {path}: {source}")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not write {path}: {source}")]
    WriteFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} contains no usable font")]
    NoFontInFile { path: PathBuf },
    #[error("the font family \"{family}\" is not available")]
    MissingFontFamily { family: String },
    #[error("the paragraph style \"{style}\" does not exist")]
    MissingParagraphStyle { style: String },
    #[error("the story \"{story}\" does not exist")]
    MissingStory { story: String },
    #[error("the page size {width} by {height} mm is not valid")]
    InvalidPageSize { width: f64, height: f64 },
    #[error("the PDF could not be written: {0}")]
    Pdf(String),
    #[error("the document could not be parsed: {0}")]
    Json(#[from] serde_json::Error),
}
