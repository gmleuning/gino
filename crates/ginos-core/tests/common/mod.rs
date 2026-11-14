//! Shared helpers for the integration tests: fixture fonts and the spike document.

use ginos_core::fonts::{fixture_font_folder, Fonts};
use ginos_core::spike::SpikeFamilies;

pub fn fixture_fonts() -> (Fonts, SpikeFamilies) {
    let folder = fixture_font_folder();
    let mut fonts = Fonts::without_system_fonts();
    let latin = fonts
        .add_file(&folder.join("CrimsonText-Regular.ttf"))
        .expect("the Latin fixture font loads");
    let korean = fonts
        .add_file(&folder.join("GowunDodum-Regular.ttf"))
        .expect("the Korean fixture font loads");
    assert_eq!(latin, vec!["Crimson Text".to_owned()]);
    assert_eq!(korean, vec!["Gowun Dodum".to_owned()]);
    let families = SpikeFamilies {
        latin: latin[0].clone(),
        korean: korean[0].clone(),
    };
    (fonts, families)
}

/// A folder under `target` for files a test writes for people to look at.
pub fn output_folder(name: &str) -> std::path::PathBuf {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join(name);
    std::fs::create_dir_all(&folder).expect("the output folder can be created");
    folder
}
