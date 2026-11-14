use std::path::PathBuf;

fn main() {
    // The Windows resource compiler cannot read a path containing an
    // apostrophe (tauri-winres escapes it as \' which RC.EXE rejects), and the
    // repository folder is called "gino's". So the window icon is copied to
    // the system temporary folder and tauri-build is pointed there.
    let icon = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("icons")
        .join("icon.ico");
    println!("cargo:rerun-if-changed={}", icon.display());
    let mut attributes = tauri_build::Attributes::new();
    if icon.exists() {
        let safe_folder = std::env::temp_dir().join("ginos-build");
        let safe_icon = safe_folder.join("icon.ico");
        let copied = std::fs::create_dir_all(&safe_folder)
            .and_then(|_| std::fs::copy(&icon, &safe_icon))
            .is_ok();
        if copied && !safe_icon.to_string_lossy().contains('\'') {
            attributes = attributes.windows_attributes(
                tauri_build::WindowsAttributes::new().window_icon_path(safe_icon),
            );
        }
    }
    tauri_build::try_build(attributes).expect("tauri-build failed; see the message above");
}
