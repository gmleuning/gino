//! A `.zine` folder on disk: `document.json` plus assets and exports.

use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::model::Document;

pub const DOCUMENT_FILE: &str = "document.json";

#[derive(Debug, Clone)]
pub struct Zine {
    pub folder: PathBuf,
    pub document: Document,
}

impl Zine {
    /// Opens a `.zine` folder, or its `document.json` directly.
    pub fn open(path: &Path) -> Result<Self, Error> {
        let (folder, file) = if path.is_dir() {
            (path.to_owned(), path.join(DOCUMENT_FILE))
        } else if path.file_name().is_some_and(|name| name == DOCUMENT_FILE) {
            (
                path.parent().map(Path::to_owned).unwrap_or_default(),
                path.to_owned(),
            )
        } else {
            return Err(Error::NotAZine {
                path: path.to_owned(),
                reason: format!("expected a folder containing {DOCUMENT_FILE}"),
            });
        };
        let text = std::fs::read_to_string(&file).map_err(|source| Error::ReadFile {
            path: file.clone(),
            source,
        })?;
        let document = Document::from_json(&text).map_err(|error| Error::NotAZine {
            path: file,
            reason: error.to_string(),
        })?;
        Ok(Self { folder, document })
    }

    /// Writes `document.json` through a temporary file and a rename, so a
    /// crash never leaves a half-written document.
    pub fn save(&self) -> Result<(), Error> {
        let file = self.folder.join(DOCUMENT_FILE);
        let temporary = self.folder.join(format!("{DOCUMENT_FILE}.tmp"));
        let json = self.document.to_json()?;
        std::fs::create_dir_all(&self.folder).map_err(|source| Error::WriteFile {
            path: self.folder.clone(),
            source,
        })?;
        std::fs::write(&temporary, json).map_err(|source| Error::WriteFile {
            path: temporary.clone(),
            source,
        })?;
        std::fs::rename(&temporary, &file).map_err(|source| Error::WriteFile {
            path: file,
            source,
        })
    }

    /// The document's font folders, resolved relative to the `.zine` folder.
    pub fn font_folders(&self) -> Vec<PathBuf> {
        self.document
            .fonts
            .folders
            .iter()
            .map(|folder| {
                let path = Path::new(folder);
                if path.is_absolute() {
                    path.to_owned()
                } else {
                    self.folder.join(path)
                }
            })
            .collect()
    }

    /// The name shown in the title bar: the folder name without `.zine`.
    pub fn title(&self) -> String {
        self.folder
            .file_name()
            .map(|name| name.to_string_lossy().trim_end_matches(".zine").to_owned())
            .unwrap_or_else(|| "Untitled".to_owned())
    }

    /// Turns an absolute path into one stored in the document: relative when
    /// the file lies inside the folder, otherwise absolute.
    pub fn stored_path(&self, absolute: &Path) -> String {
        match absolute.strip_prefix(&self.folder) {
            Ok(relative) => relative.to_string_lossy().into_owned(),
            Err(_) => absolute.to_string_lossy().into_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spike::{spike_document, SpikeFamilies};

    #[test]
    fn save_and_open_round_trip() {
        let folder = std::env::temp_dir().join(format!("ginos-zine-test-{}.zine", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        let document = spike_document(&SpikeFamilies {
            latin: "A".to_owned(),
            korean: "B".to_owned(),
        });
        let zine = Zine {
            folder: folder.clone(),
            document: document.clone(),
        };
        zine.save().unwrap();
        assert!(!folder.join("document.json.tmp").exists());
        let reopened = Zine::open(&folder).unwrap();
        assert_eq!(reopened.document, document);
        assert_eq!(reopened.title(), format!("ginos-zine-test-{}", std::process::id()));
        assert_eq!(reopened.stored_path(&folder.join("assets").join("a.png")), "assets\\a.png");
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn a_folder_without_a_document_is_refused() {
        let error = Zine::open(&std::env::temp_dir()).unwrap_err();
        assert!(matches!(error, Error::ReadFile { .. }));
        let error = Zine::open(Path::new("C:\\nowhere\\picture.png")).unwrap_err();
        assert!(matches!(error, Error::NotAZine { .. }));
    }
}
