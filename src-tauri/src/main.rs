//! Gino's desktop shell. The interface sends intents; every command here
//! forwards to the core's [`Session`], which owns the document, and returns
//! what changed. No layout happens in this crate.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ginos_core::compose::Overflow;
use ginos_core::fonts::Fonts;
use ginos_core::model::{CharacterStyle, ImageFit, ParagraphStyle, Rect, Settings, Size};
use ginos_core::presets::{default_margins, fold_sizes, NewDocument};
use ginos_core::session::{Arrange, DocumentView, Motion, Session, Tool, Update};
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

/// System fonts, plus the fixture fonts so sample documents open anywhere.
fn fonts() -> Fonts {
    let mut fonts = Fonts::with_system_fonts();
    fonts.add_folder(&ginos_core::fonts::fixture_font_folder());
    fonts
}

fn install(state: &State<'_, AppState>, mut session: Session) -> CommandResult<DocumentView> {
    let view = session.view().map_err(|error| error.to_string())?;
    let mut guard = state.session.lock().map_err(|_| "the document is busy".to_owned())?;
    *guard = Some(session);
    Ok(view)
}

#[tauri::command]
fn open_document(path: String, state: State<'_, AppState>) -> CommandResult<DocumentView> {
    let session = Session::open(Path::new(&path), fonts()).map_err(|error| error.to_string())?;
    install(&state, session)
}

#[tauri::command]
fn new_document(path: String, request: NewDocument, state: State<'_, AppState>) -> CommandResult<DocumentView> {
    let session = Session::create(Path::new(&path), &request, fonts()).map_err(|error| error.to_string())?;
    install(&state, session)
}

/// Defaults for the new-document dialog, per fold: trim, sheet and margins.
#[derive(serde::Serialize)]
struct FoldDefaults {
    trim: Size,
    sheet: Size,
    margins: ginos_core::model::Margins,
}

#[tauri::command]
fn fold_defaults(fold: ginos_core::model::Fold, custom_trim: Option<Size>) -> FoldDefaults {
    let (trim, sheet) = fold_sizes(fold, custom_trim);
    FoldDefaults {
        trim,
        sheet,
        margins: default_margins(fold),
    }
}

#[tauri::command]
fn document_view(state: State<'_, AppState>) -> CommandResult<DocumentView> {
    with_session(&state, |session| session.view())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn pointer_down(
    page: usize,
    x: f64,
    y: f64,
    tolerance: f64,
    tool: Tool,
    shift: bool,
    clicks: u32,
    state: State<'_, AppState>,
) -> CommandResult<Update> {
    with_session(&state, |session| session.pointer_down(page, x, y, tolerance, tool, shift, clicks))
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
fn set_frame_appearance(
    page: usize,
    frame_id: String,
    stroke_width: f64,
    grey: f64,
    filled: bool,
    state: State<'_, AppState>,
) -> CommandResult<Update> {
    with_session(&state, |session| session.set_frame_appearance(page, &frame_id, stroke_width, grey, filled))
}

#[tauri::command]
fn nudge(dx: f64, dy: f64, state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.nudge(dx, dy))
}

#[tauri::command]
fn delete_frame(state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.delete_frame())
}

#[tauri::command]
fn arrange(arrange: Arrange, state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.arrange(arrange))
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
fn thread_to(page: usize, x: f64, y: f64, state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.thread_to(page, x, y))
}

#[tauri::command]
fn break_thread_after(state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.break_thread_after())
}

// Text --------------------------------------------------------------------

#[tauri::command]
fn edit_frame(page: usize, frame_id: String, state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.edit_frame(page, &frame_id))
}

#[tauri::command]
fn end_editing(state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.end_editing())
}

#[tauri::command]
fn insert_text(text: String, state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.insert_text(&text))
}

#[tauri::command]
fn delete_text(backward: bool, word: bool, state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.delete(backward, word))
}

#[tauri::command]
fn set_preedit(text: String, state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.set_preedit(&text))
}

#[tauri::command]
fn commit_text(text: String, state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.commit_text(&text))
}

#[tauri::command]
fn move_caret(motion: Motion, extend: bool, state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.move_caret(motion, extend))
}

#[tauri::command]
fn select_all(state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.select_all())
}

// Styles ------------------------------------------------------------------

#[tauri::command]
fn apply_paragraph_style(style: String, state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.apply_paragraph_style(&style))
}

#[tauri::command]
fn apply_character_style(style: Option<String>, state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.apply_character_style(style.as_deref()))
}

#[tauri::command]
fn update_paragraph_style(style: ParagraphStyle, state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.update_paragraph_style(style))
}

#[tauri::command]
fn create_paragraph_style(id: String, based_on: String, state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.create_paragraph_style(&id, &based_on))
}

#[tauri::command]
fn update_character_style(style: CharacterStyle, state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.update_character_style(style))
}

#[tauri::command]
fn create_character_style(id: String, state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.create_character_style(&id))
}

// Pages, settings, history, files ----------------------------------------

#[tauri::command]
fn add_pages(count: usize, state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.add_pages(count))
}

#[tauri::command]
fn remove_pages(count: usize, state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.remove_pages(count))
}

#[tauri::command]
fn set_settings(settings: Settings, state: State<'_, AppState>) -> CommandResult<Update> {
    with_session(&state, |session| session.set_settings(settings))
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
fn export_imposed(path: String, sheet: Option<Size>, state: State<'_, AppState>) -> CommandResult<ExportResult> {
    with_session(&state, |session| {
        let target = PathBuf::from(path);
        let overflows = session.export_imposed_pdf(&target, sheet)?;
        Ok(ExportResult {
            path: target.to_string_lossy().into_owned(),
            overflows,
        })
    })
}

#[tauri::command]
fn default_export_path(suffix: String, state: State<'_, AppState>) -> CommandResult<String> {
    with_session(&state, |session| {
        Ok(session.default_export_path(&suffix).to_string_lossy().into_owned())
    })
}

#[tauri::command]
fn font_families(state: State<'_, AppState>) -> CommandResult<Vec<String>> {
    let mut guard = state.session.lock().map_err(|_| "the document is busy".to_owned())?;
    match guard.as_mut() {
        Some(session) => Ok(session.fonts.family_names()),
        // Before any document is open (the new-document dialog), list from a fresh font set.
        None => Ok(fonts().family_names()),
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            open_document,
            new_document,
            fold_defaults,
            document_view,
            pointer_down,
            pointer_move,
            pointer_up,
            cancel_drag,
            select_frame,
            set_frame_rect,
            set_frame_appearance,
            nudge,
            delete_frame,
            arrange,
            place_image,
            set_image_fit,
            adjust_crop,
            thread_to,
            break_thread_after,
            edit_frame,
            end_editing,
            insert_text,
            delete_text,
            set_preedit,
            commit_text,
            move_caret,
            select_all,
            apply_paragraph_style,
            apply_character_style,
            update_paragraph_style,
            create_paragraph_style,
            update_character_style,
            create_character_style,
            add_pages,
            remove_pages,
            set_settings,
            undo,
            redo,
            save_document,
            export_reader,
            export_imposed,
            default_export_path,
            font_families,
        ])
        .run(tauri::generate_context!())
        .expect("Gino's could not start its window");
}
