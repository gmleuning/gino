// Calls into the core through Tauri. Every function returns what the core
// says changed; nothing here computes a page position.

import { invoke } from "@tauri-apps/api/core";

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export type FrameKind = "text" | "image" | "rectangle" | "rule" | "page_number";
export type ImageFit = "fill" | "fit" | "manual";

export interface FramePatch {
  page: number;
  frame_id: string;
  kind: FrameKind;
  rect: Rect;
  svg: string;
}

export interface SelectionInfo {
  page: number;
  frame_id: string;
  kind: FrameKind;
  rect: Rect;
  image: string | null;
  fit: ImageFit;
  story: string | null;
  thread_index: number | null;
}

export interface SnapFeedback {
  page: number;
  x: number | null;
  y: number | null;
}

export interface Overflow {
  story: string;
  characters: number;
}

export interface ImageProblem {
  page: number;
  frame: string;
  message: string;
}

export interface Update {
  patches: FramePatch[];
  selection: SelectionInfo | null;
  snap: SnapFeedback | null;
  overflows: Overflow[];
  image_problems: ImageProblem[];
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
  pages: PageView[];
  spreads: SpreadView[];
  update: Update;
}

export interface ExportResult {
  path: string;
  overflows: Overflow[];
}

export interface ImeTestView {
  svg: string;
  caret_x: number;
  caret_top: number;
  caret_bottom: number;
}

export const api = {
  openDocument: (path: string) => invoke<DocumentView>("open_document", { path }),
  pointerDown: (page: number, x: number, y: number, tolerance: number) =>
    invoke<Update>("pointer_down", { page, x, y, tolerance }),
  pointerMove: (x: number, y: number) => invoke<Update>("pointer_move", { x, y }),
  pointerUp: () => invoke<Update>("pointer_up"),
  cancelDrag: () => invoke<Update>("cancel_drag"),
  selectFrame: (page: number, frameId: string | null) =>
    invoke<Update>("select_frame", { page, frameId }),
  setFrameRect: (page: number, frameId: string, rect: Rect) =>
    invoke<Update>("set_frame_rect", { page, frameId, rect }),
  nudge: (dx: number, dy: number) => invoke<Update>("nudge", { dx, dy }),
  placeImage: (page: number, frameId: string, path: string) =>
    invoke<Update>("place_image", { page, frameId, path }),
  setImageFit: (page: number, frameId: string, fit: ImageFit) =>
    invoke<Update>("set_image_fit", { page, frameId, fit }),
  adjustCrop: (page: number, frameId: string, dx: number, dy: number, factor: number) =>
    invoke<Update>("adjust_crop", { page, frameId, dx, dy, factor }),
  undo: () => invoke<Update>("undo"),
  redo: () => invoke<Update>("redo"),
  save: () => invoke<Update>("save_document"),
  exportReader: (path: string | null) => invoke<ExportResult>("export_reader", { path }),
  defaultExportPath: () => invoke<string>("default_export_path"),
  imeTest: (committed: string, composing: string, width: number) =>
    invoke<ImeTestView>("ime_test", { committed, composing, width }),
  fontFamilies: () => invoke<string[]>("font_families"),
};
