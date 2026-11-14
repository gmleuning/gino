//! Font discovery and loading. One [`Fonts`] value holds the shared Parley
//! font context (the font database) and layout context (scratch space); the
//! whole program shares a single one.

use std::path::Path;
use std::sync::Arc;

use fontique::{Blob, Collection, CollectionOptions, SourceCache};
use parley::{FontContext, LayoutContext};

use crate::error::Error;
use crate::scene::Rgb;

pub struct Fonts {
    pub context: FontContext,
    pub layout: LayoutContext<Rgb>,
}

impl Fonts {
    /// Fonts installed in Windows, plus whatever is added later.
    pub fn with_system_fonts() -> Self {
        Self {
            context: FontContext::new(),
            layout: LayoutContext::new(),
        }
    }

    /// Only fonts added explicitly. Used by tests so results do not depend on the machine.
    pub fn without_system_fonts() -> Self {
        let collection = Collection::new(CollectionOptions {
            shared: false,
            system_fonts: false,
        });
        Self {
            context: FontContext {
                collection,
                source_cache: SourceCache::default(),
            },
            layout: LayoutContext::new(),
        }
    }

    /// Registers one font file and returns the family names it added.
    pub fn add_file(&mut self, path: &Path) -> Result<Vec<String>, Error> {
        let bytes = std::fs::read(path).map_err(|source| Error::ReadFile {
            path: path.to_owned(),
            source,
        })?;
        let blob = Blob::new(Arc::new(bytes));
        let registered = self.context.collection.register_fonts(blob, None);
        if registered.is_empty() {
            return Err(Error::NoFontInFile {
                path: path.to_owned(),
            });
        }
        let mut names = Vec::new();
        for (family_id, _) in registered {
            if let Some(name) = self.context.collection.family_name(family_id) {
                if !names.iter().any(|known| known == name) {
                    names.push(name.to_owned());
                }
            }
        }
        Ok(names)
    }

    /// Registers every font file found in a folder (a user font folder).
    pub fn add_folder(&mut self, path: &Path) {
        self.context.collection.load_fonts_from_paths([path]);
    }

    /// Every family name known to the font database, sorted, for the font pickers.
    pub fn family_names(&mut self) -> Vec<String> {
        let mut names: Vec<String> = self
            .context
            .collection
            .family_names()
            .map(str::to_owned)
            .collect();
        names.sort();
        names
    }

    pub fn has_family(&mut self, name: &str) -> bool {
        self.context.collection.family_id(name).is_some()
    }

    /// Checks that a family exists, with a plain error otherwise.
    pub fn require_family(&mut self, name: &str) -> Result<(), Error> {
        if self.has_family(name) {
            Ok(())
        } else {
            Err(Error::MissingFontFamily {
                family: name.to_owned(),
            })
        }
    }
}

/// Path of the fixture font folder inside the repository.
pub fn fixture_font_folder() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("fonts")
}
