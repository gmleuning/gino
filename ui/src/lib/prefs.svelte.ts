// Preferences belong to this computer, not to a document. The shell stores
// them as JSON in the app's config folder; this module owns their shape and
// fills in defaults for anything missing or from an older version.

import { api } from "./api";
import { DEFAULT_SHORTCUTS, type CommandId } from "./commands";

export type Theme = "system" | "light" | "dark";

export interface Preferences {
  theme: Theme;
  /** Interface font family; empty for the system's. */
  ui_font: string;
  /** A picture for the desk behind the pages; empty for Gino's own. */
  desk_image: string;
  /** Interface text size in pixels. */
  ui_font_size: number;
  /** New zines start with cover image frames and a threaded story. */
  starter_frames: boolean;
  /** Show the Korean controls: Korean font, scale and line breaking. */
  show_korean: boolean;
  new_latin_font: string;
  new_korean_font: string;
  new_size: number;
  new_leading: number;
  /** What is shown when Gino starts. */
  show_guides: boolean;
  show_crop_marks: boolean;
  show_outlines: boolean;
  show_bleed: boolean;
  show_pages: boolean;
  show_panel: boolean;
  /** Images finer than this are resampled down when printing or exporting; 0 keeps every pixel. */
  print_dpi: number;
  shortcuts: Record<CommandId, string[]>;
}

export const DEFAULT_PREFERENCES: Preferences = {
  theme: "system",
  ui_font: "",
  desk_image: "",
  ui_font_size: 13,
  starter_frames: true,
  show_korean: false,
  new_latin_font: "Crimson Text",
  new_korean_font: "Gowun Dodum",
  new_size: 10,
  new_leading: 14,
  show_guides: false,
  show_crop_marks: true,
  show_outlines: false,
  show_bleed: false,
  show_pages: true,
  show_panel: true,
  print_dpi: 300,
  shortcuts: DEFAULT_SHORTCUTS,
};

function merge(stored: Record<string, unknown>): Preferences {
  const result = structuredClone(DEFAULT_PREFERENCES);
  for (const key of Object.keys(DEFAULT_PREFERENCES) as (keyof Preferences)[]) {
    if (key === "shortcuts") continue;
    const value = stored[key];
    if (value !== undefined && typeof value === typeof DEFAULT_PREFERENCES[key]) {
      (result as unknown as Record<string, unknown>)[key] = value;
    }
  }
  const shortcuts = stored.shortcuts as Record<string, unknown> | undefined;
  if (shortcuts && typeof shortcuts === "object") {
    for (const id of Object.keys(DEFAULT_SHORTCUTS) as CommandId[]) {
      const keys = shortcuts[id];
      if (Array.isArray(keys) && keys.every((key) => typeof key === "string")) result.shortcuts[id] = keys;
    }
  }
  return result;
}

class PreferenceStore {
  current = $state<Preferences>(structuredClone(DEFAULT_PREFERENCES));

  async load() {
    try {
      this.current = merge((await api.getPreferences()) as Record<string, unknown>);
    } catch {
      this.current = structuredClone(DEFAULT_PREFERENCES);
    }
    applyAppearance(this.current);
    return this.current;
  }

  async save(next: Preferences) {
    this.current = structuredClone(next);
    applyAppearance(this.current);
    await api.setPreferences($state.snapshot(this.current));
  }

  /** The command a shortcut runs, if any. */
  commandFor(combo: string): CommandId | null {
    for (const [id, keys] of Object.entries(this.current.shortcuts)) {
      if (keys.includes(combo)) return id as CommandId;
    }
    return null;
  }
}

export const prefs = new PreferenceStore();

let systemDark: MediaQueryList | null = null;

/** Sets the colour scheme and interface font on the page. */
export function applyAppearance(preferences: Preferences) {
  const root = document.documentElement;
  systemDark ??= window.matchMedia("(prefers-color-scheme: dark)");
  const dark = preferences.theme === "dark" || (preferences.theme === "system" && systemDark.matches);
  root.dataset.theme = dark ? "dark" : "light";
  root.style.setProperty(
    "--ui-font",
    preferences.ui_font ? `"${preferences.ui_font}", "Malgun Gothic", system-ui, sans-serif` : `"Segoe UI", "Malgun Gothic", system-ui, sans-serif`,
  );
  root.style.setProperty("--ui-size", `${preferences.ui_font_size}px`);
  applyDeskImage(preferences.desk_image);
}

let deskShown = "";

/** Puts the chosen picture behind the pages, or Gino's own field when none is chosen. */
async function applyDeskImage(path: string) {
  if (path === deskShown) return;
  deskShown = path;
  const root = document.documentElement;
  if (!path) {
    root.style.removeProperty("--desk-grain");
    root.style.removeProperty("--desk-size");
    return;
  }
  try {
    const url = await api.deskBackground(path);
    if (deskShown !== path) return;
    root.style.setProperty("--desk-grain", `url("${url}")`);
    root.style.setProperty("--desk-size", "cover");
  } catch {
    // A picture that has moved or will not open: keep Gino's own field.
    deskShown = "";
    root.style.removeProperty("--desk-grain");
    root.style.removeProperty("--desk-size");
  }
}

/** Follow Windows when the theme is "system". */
export function watchSystemTheme() {
  systemDark ??= window.matchMedia("(prefers-color-scheme: dark)");
  systemDark.addEventListener("change", () => applyAppearance(prefs.current));
}
