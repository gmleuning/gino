// Interface state. The core owns the document; this holds only what the
// core last sent and how it is being shown (zoom, current spread, tool).

import {
  api,
  type CharacterStyle,
  type DocumentView,
  type FramePatch,
  type ImageProblem,
  type Overflow,
  type ParagraphStyle,
  type SelectionInfo,
  type SnapFeedback,
  type TextView,
  type Tool,
  type Update,
} from "./api";

export const frameKey = (page: number, frameId: string) => `${page}\u0000${frameId}`;

class AppState {
  view = $state<DocumentView | null>(null);
  frames = $state<Map<string, FramePatch>>(new Map());
  selection = $state<SelectionInfo | null>(null);
  text = $state<TextView | null>(null);
  snap = $state<SnapFeedback | null>(null);
  overflows = $state<Overflow[]>([]);
  imageProblems = $state<ImageProblem[]>([]);
  paragraphStyles = $state<ParagraphStyle[]>([]);
  characterStyles = $state<CharacterStyle[]>([]);
  dirty = $state(false);
  canUndo = $state(false);
  canRedo = $state(false);
  currentSpread = $state(0);
  /** Screen pixels per millimetre. */
  zoom = $state(3);
  message = $state<string | null>(null);
  showGuides = $state(true);
  tool = $state<Tool>("select");
  /** The next canvas click continues the selected story into the clicked frame. */
  threading = $state(false);
  fontFamilies = $state<string[]>([]);

  openView(view: DocumentView) {
    this.view = view;
    this.frames = new Map();
    this.currentSpread = Math.min(this.currentSpread, Math.max(view.spreads.length - 1, 0));
    this.applyUpdate(view.update);
  }

  applyUpdate(update: Update) {
    if (update.patches.length > 0) {
      const next = new Map(this.frames);
      for (const patch of update.patches) {
        next.set(frameKey(patch.page, patch.frame_id), patch);
      }
      this.frames = next;
    }
    if (update.page_frames.length > 0 && this.view) {
      const pages = this.view.pages.slice();
      for (const entry of update.page_frames) {
        if (pages[entry.page]) pages[entry.page] = { ...pages[entry.page], frame_ids: entry.frame_ids };
      }
      this.view = { ...this.view, pages };
    }
    this.selection = update.selection;
    this.text = update.text;
    this.snap = update.snap;
    this.overflows = update.overflows;
    this.imageProblems = update.image_problems;
    this.paragraphStyles = update.paragraph_styles;
    this.characterStyles = update.character_styles;
    this.dirty = update.dirty;
    this.canUndo = update.can_undo;
    this.canRedo = update.can_redo;
    if (update.view_changed) {
      // Pages or settings changed: fetch the whole view again.
      api
        .documentView()
        .then((view) => this.openView(view))
        .catch((error) => this.say(String(error)));
    }
  }

  spreadOfPage(page: number): number {
    const spreads = this.view?.spreads ?? [];
    const index = spreads.findIndex((spread) => spread.pages.some(([p]) => p === page));
    return index < 0 ? 0 : index;
  }

  say(message: string | null) {
    this.message = message;
  }
}

export const app = new AppState();

/** Runs a core call and shows its error as a plain sentence. */
export async function run(action: () => Promise<Update>): Promise<Update | null> {
  try {
    const update = await action();
    app.applyUpdate(update);
    return update;
  } catch (error) {
    app.say(String(error));
    return null;
  }
}
