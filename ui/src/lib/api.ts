// Calls into the core through Tauri. Every function returns what the core
// says changed; nothing here computes a page position or a line break.

import { invoke } from "@tauri-apps/api/core";

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Size {
  width: number;
  height: number;
}

export interface Margins {
  top: number;
  bottom: number;
  inside: number;
  outside: number;
}

export interface Columns {
  count: number;
  gutter: number;
}

export type Fold = "saddle_a5_on_a4" | "saddle_a6_on_a5" | "eight_page_fold" | "custom";
export type SheetMode = "same_size" | "oversize";
export type FrameKind = "text" | "image" | "rectangle" | "rule" | "page_number";
export type ImageFit = "fill" | "fit" | "manual";
export type Tool = "select" | "text" | "image" | "rectangle" | "rule";
export type Motion =
  | "left"
  | "right"
  | "up"
  | "down"
  | "word_left"
  | "word_right"
  | "line_start"
  | "line_end"
  | "story_start"
  | "story_end";
export type Arrange = "forward" | "backward" | "front" | "back";
export type Alignment = "left" | "center" | "right" | "justify";
export type KoreanBreak = "syllable" | "word";

export interface Settings {
  fold: Fold;
  trim: Size;
  sheet: Size;
  bleed: number;
  printer_margin: number;
  margins: Margins;
  columns: Columns;
  creep_per_sheet: number;
  page_count: number;
}

export interface ParagraphStyle {
  id: string;
  latin_font: string;
  korean_font: string;
  korean_scale: number;
  size: number;
  leading: number;
  tracking: number;
  alignment: Alignment;
  space_before: number;
  space_after: number;
  first_line_indent: number;
  korean_break: KoreanBreak;
}

export interface CharacterStyle {
  id: string;
  font_style?: "normal" | "italic";
  font_weight?: number;
  underline?: boolean;
}

export interface FramePatch {
  page: number;
  frame_id: string;
  kind: FrameKind;
  rect: Rect;
  svg: string;
}

export interface PageFrames {
  page: number;
  frame_ids: string[];
}

export interface SelectionInfo {
  page: number;
  frame_id: string;
  kind: FrameKind;
  rect: Rect;
  image: string | null;
  fit: ImageFit;
  story: string | null;
  thread_position: number;
  thread_length: number;
  stack_index: number;
  stack_count: number;
  stroke_width: number;
  grey: number;
  filled: boolean;
}

export interface Caret {
  page: number;
  x: number;
  top: number;
  bottom: number;
}

export interface SelectionBox {
  page: number;
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface TextView {
  story: string;
  caret: Caret | null;
  selection: SelectionBox[];
  selected_text: string;
  composing: boolean;
  paragraph_style: string;
  character_style: string | null;
}

export interface SnapFeedback {
  page: number;
  x: number | null;
  y: number | null;
}

export interface Overflow {
  story: string;
  characters: number;
  page: number;
  frame: string;
}

export interface ImageProblem {
  page: number;
  frame: string;
  message: string;
}

export interface Update {
  patches: FramePatch[];
  page_frames: PageFrames[];
  view_changed: boolean;
  selection: SelectionInfo | null;
  text: TextView | null;
  snap: SnapFeedback | null;
  overflows: Overflow[];
  image_problems: ImageProblem[];
  paragraph_styles: ParagraphStyle[];
  character_styles: CharacterStyle[];
  dirty: boolean;
  can_undo: boolean;
  can_redo: boolean;
}

export interface PageView {
  index: number;
  width: number;
  height: number;
  guides_svg: string;
  frame_ids: string[];
}

export interface SpreadView {
  pages: [number, number][];
  width: number;
  height: number;
}

export interface DocumentView {
  title: string;
  folder: string;
  settings: Settings;
  sheet_mode: SheetMode | null;
  pages: PageView[];
  spreads: SpreadView[];
  update: Update;
}

export interface ExportResult {
  path: string;
  overflows: Overflow[];
}

export interface NewDocument {
  fold: Fold;
  custom_trim: Size | null;
  page_count: number;
  sheet: Size | null;
  margins: Margins;
  columns: Columns;
  latin_font: string;
  korean_font: string;
  size: number;
  leading: number;
  starter_frames: boolean;
}

export interface Preferences {
  starter_frames: boolean;
}

export interface FoldDefaults {
  trim: Size;
  sheet: Size;
  margins: Margins;
}

export const api = {
  openDocument: (path: string) => invoke<DocumentView>("open_document", { path }),
  newDocument: (path: string, request: NewDocument) => invoke<DocumentView>("new_document", { path, request }),
  foldDefaults: (fold: Fold, customTrim: Size | null) =>
    invoke<FoldDefaults>("fold_defaults", { fold, customTrim }),
  documentView: () => invoke<DocumentView>("document_view"),

  pointerDown: (page: number, x: number, y: number, tolerance: number, tool: Tool, shift: boolean, clicks: number) =>
    invoke<Update>("pointer_down", { page, x, y, tolerance, tool, shift, clicks }),
  pointerMove: (x: number, y: number) => invoke<Update>("pointer_move", { x, y }),
  pointerUp: () => invoke<Update>("pointer_up"),
  cancelDrag: () => invoke<Update>("cancel_drag"),
  selectFrame: (page: number, frameId: string | null) => invoke<Update>("select_frame", { page, frameId }),
  setFrameRect: (page: number, frameId: string, rect: Rect) =>
    invoke<Update>("set_frame_rect", { page, frameId, rect }),
  setFrameAppearance: (page: number, frameId: string, strokeWidth: number, grey: number, filled: boolean) =>
    invoke<Update>("set_frame_appearance", { page, frameId, strokeWidth, grey, filled }),
  nudge: (dx: number, dy: number) => invoke<Update>("nudge", { dx, dy }),
  deleteFrame: () => invoke<Update>("delete_frame"),
  arrange: (arrange: Arrange) => invoke<Update>("arrange", { arrange }),
  placeImage: (page: number, frameId: string, path: string) =>
    invoke<Update>("place_image", { page, frameId, path }),
  setImageFit: (page: number, frameId: string, fit: ImageFit) =>
    invoke<Update>("set_image_fit", { page, frameId, fit }),
  adjustCrop: (page: number, frameId: string, dx: number, dy: number, factor: number) =>
    invoke<Update>("adjust_crop", { page, frameId, dx, dy, factor }),
  threadTo: (page: number, x: number, y: number) => invoke<Update>("thread_to", { page, x, y }),
  breakThreadAfter: () => invoke<Update>("break_thread_after"),

  editFrame: (page: number, frameId: string) => invoke<Update>("edit_frame", { page, frameId }),
  endEditing: () => invoke<Update>("end_editing"),
  insertText: (text: string) => invoke<Update>("insert_text", { text }),
  deleteText: (backward: boolean, word: boolean) => invoke<Update>("delete_text", { backward, word }),
  setPreedit: (text: string) => invoke<Update>("set_preedit", { text }),
  commitText: (text: string) => invoke<Update>("commit_text", { text }),
  moveCaret: (motion: Motion, extend: boolean) => invoke<Update>("move_caret", { motion, extend }),
  selectAll: () => invoke<Update>("select_all"),

  applyParagraphStyle: (style: string) => invoke<Update>("apply_paragraph_style", { style }),
  applyCharacterStyle: (style: string | null) => invoke<Update>("apply_character_style", { style }),
  updateParagraphStyle: (style: ParagraphStyle) => invoke<Update>("update_paragraph_style", { style }),
  createParagraphStyle: (id: string, basedOn: string) =>
    invoke<Update>("create_paragraph_style", { id, basedOn }),
  updateCharacterStyle: (style: CharacterStyle) => invoke<Update>("update_character_style", { style }),
  createCharacterStyle: (id: string) => invoke<Update>("create_character_style", { id }),

  addPages: (count: number) => invoke<Update>("add_pages", { count }),
  removePages: (count: number) => invoke<Update>("remove_pages", { count }),
  setSettings: (settings: Settings) => invoke<Update>("set_settings", { settings }),

  undo: () => invoke<Update>("undo"),
  redo: () => invoke<Update>("redo"),
  save: () => invoke<Update>("save_document"),
  exportReader: (path: string | null) => invoke<ExportResult>("export_reader", { path }),
  exportImposed: (path: string, sheet: Size | null) => invoke<ExportResult>("export_imposed", { path, sheet }),
  exportPage: (path: string, page: number) => invoke<ExportResult>("export_page", { path, page }),
  exportPaged: (path: string) => invoke<ExportResult>("export_paged", { path }),
  openInDefaultApp: (path: string) => invoke<void>("open_in_default_app", { path }),
  getPreferences: () => invoke<Preferences>("get_preferences"),
  setPreferences: (preferences: Preferences) => invoke<void>("set_preferences", { preferences }),
  defaultExportPath: (suffix: string) => invoke<string>("default_export_path", { suffix }),
  fontFamilies: () => invoke<string[]>("font_families"),
};
