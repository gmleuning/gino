// Interface state. The core owns the document; this holds only what the
// core last sent and how it is being shown (zoom, current spread).

import type { DocumentView, FramePatch, SelectionInfo, SnapFeedback, Update, Overflow, ImageProblem } from "./api";

export const frameKey = (page: number, frameId: string) => `${page}\u0000${frameId}`;

class AppState {
  view = $state<DocumentView | null>(null);
  frames = $state<Map<string, FramePatch>>(new Map());
  selection = $state<SelectionInfo | null>(null);
  snap = $state<SnapFeedback | null>(null);
  overflows = $state<Overflow[]>([]);
  imageProblems = $state<ImageProblem[]>([]);
  dirty = $state(false);
  canUndo = $state(false);
  canRedo = $state(false);
  currentSpread = $state(0);
  /** Screen pixels per millimetre. */
  zoom = $state(3);
  message = $state<string | null>(null);
  showGuides = $state(true);
  showImeTest = $state(false);

  openView(view: DocumentView) {
    this.view = view;
    this.frames = new Map();
    this.currentSpread = 0;
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
    this.selection = update.selection;
    this.snap = update.snap;
    this.overflows = update.overflows;
    this.imageProblems = update.image_problems;
    this.dirty = update.dirty;
    this.canUndo = update.can_undo;
    this.canRedo = update.can_redo;
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
