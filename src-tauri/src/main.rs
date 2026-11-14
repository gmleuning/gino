//! Gino's desktop shell. The interface sends intents; every command here
//! forwards to the core's [`Session`], which owns the document, and returns
//! what changed. No layout happens in this crate.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ginos_core::compose::Overflow;
use ginos_core::fonts::Fonts;
use ginos_core::model::{ImageFit, Rect};
use ginos_core::session::{DocumentView, ImeTestView, Session, Update};
use tauri::State;

#[derive(Default)]
struct AppState {
    session: Mutex<Option<Session>>,
}

type CommandResult<T> = Result<T, String>;

fn with_session<T>(
    state: &State<'_, AppState>,
    f: impl FnOnce(&mut Session) -> Result<T, ginos_core::Error>,
) -> CommandResult<T> {
    let mut guard = state.session.lock().map_err(|_| "the document is busy".to_owned())?;
    let session = guard.as_mut().ok_or_else(|| ginos_core::Error::NoDocument.to_string())?;
    f(session).map_err(|error| error.to_string())
}

#[tauri::command]
fn open_document(path: String, state: State<'_, AppState>) -> CommandResult<DocumentView> {
    let mut fonts = Fonts::with_system_fonts();
    // The fixture fonts are always available, so sample documents open anywhere.
    fonts.add_folder(&ginos_core::fonts::fixture_font_folder());
    let mut session = Session::open(Path::new(&path), fonts).map_err(|error| error.to_string())?;
    let view = session.view().map_err(|error| error.to_string())?;
    let mut guard = state.session.lock().map_err(|_| "the document is busy".to_owned())?;
    *guard = Some(session);
    Ok(view)
}

#[tauri::command]
fn pointer_down(page: usize, x: f64, y: f64, tolerance: f64, state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.pointer_down(page, x, y, tolerance))
}

#[tauri::command]
fn pointer_move(x: f64, y: f64, state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.pointer_move(x, y))
}

#[tauri::command]
fn pointer_up(state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.pointer_up())
}

#[tauri::command]
fn cancel_drag(state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.cancel_drag())
}

#[tauri::command]
fn select_frame(page: usize, frame_id: Option<String>, state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.select(page, frame_id.as_deref()))
}

#[tauri::command]
fn set_frame_rect(page: usize, frame_id: String, rect: Rect, state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.set_frame_rect(page, &frame_id, rect))
}

#[tauri::command]
fn nudge(dx: f64, dy: f64, state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.nudge(dx, dy))
}

#[tauri::command]
fn place_image(page: usize, frame_id: String, path: String, state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.place_image(page, &frame_id, Path::new(&path)))
}

#[tauri::command]
fn set_image_fit(page: usize, frame_id: String, fit: ImageFit, state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.set_image_fit(page, &frame_id, fit))
}

#[tauri::command]
fn adjust_crop(
    page: usize,
    frame_id: String,
    dx: f64,
    dy: f64,
    factor: f64,
    state: State<'_, AppState>,
) -> CommandResult<Update> {
    with_session(&state, |session| session.adjust_crop(page, &frame_id, dx, dy, factor))
}

#[tauri::command]
fn undo(state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.undo())
}

#[tauri::command]
fn redo(state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.redo())
}

#[tauri::command]
fn save_document(state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.save())
}

#[derive(serde::Serialize)]
struct ExportResult {
    path: String,
    overflows: Vec<Overflow>,
}

#[tauri::command]
fn export_reader(path: Option<String>, state: State<'_, AppState>) -> CommandResult<ExportResult> {
    with_session(&state, |session| {
        let target: PathBuf = match path {
            Some(path) => PathBuf::from(path),
            None => session.default_export_path("reader"),
        };
        let overflows = session.export_reader_pdf(&target)?;
        Ok(ExportResult {
            path: target.to_string_lossy().into_owned(),
            overflows,
        })
    })
}

#[tauri::command]
fn default_export_path(state: State<'_, AppState>) -> CommandResult<String> {
    with_session(&state, |session| {
        Ok(session.default_export_path("reader").to_string_lossy().into_owned())
    })
}

#[tauri::command]
fn ime_test(committed: String, composing: String, width: f64, state: State<'_, AppState>) -> CommandResult<ImeTestView> {
    with_session(&state, |session| session.ime_test(&committed, &composing, width))
}

#[tauri::command]
fn font_families(state: State<'_, AppState>) -> CommandResult<Vec<String>> {
    with_session(&state, |session| Ok(session.fonts.family_names()))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            open_document,
            pointer_down,
            pointer_move,
            pointer_up,
            cancel_drag,
            select_frame,
            set_frame_rect,
            nudge,
            place_image,
            set_image_fit,
            adjust_crop,
            undo,
            redo,
            save_document,
            export_reader,
            default_export_path,
            ime_test,
            font_families,
        ])
        .run(tauri::generate_context!())
        .expect("Gino's could not start its window");
}
