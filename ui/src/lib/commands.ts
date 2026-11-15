// Every command the menus offer, with its default shortcut keys. A key is
// stored by its physical position (KeyboardEvent.code, e.g. "KeyN"), so
// shortcuts work the same whether the keyboard is typing English or Korean.

export type CommandId =
  | "new"
  | "open"
  | "save"
  | "undo"
  | "redo"
  | "exportReader"
  | "exportPage"
  | "shortcuts"
  | "preferences"
  | "toggleGuides"
  | "toggleCropMarks"
  | "toggleOutlines"
  | "toggleBleed"
  | "togglePages"
  | "togglePanel"
  | "zoomIn"
  | "zoomOut"
  | "fit"
  | "printPaged"
  | "printViewed"
  | "printFolded"
  | "toolSelect"
  | "toolText"
  | "toolImage"
  | "toolLine";

export interface CommandInfo {
  id: CommandId;
  label: string;
  group: string;
  keys: string[];
}

export const COMMANDS: CommandInfo[] = [
  { id: "new", label: "New", group: "File", keys: ["Ctrl+KeyN"] },
  { id: "open", label: "Open", group: "File", keys: ["Ctrl+KeyO"] },
  { id: "save", label: "Save", group: "File", keys: ["Ctrl+KeyS"] },
  { id: "undo", label: "Undo", group: "Edit", keys: ["Ctrl+KeyZ"] },
  { id: "redo", label: "Redo", group: "Edit", keys: ["Ctrl+KeyY", "Ctrl+Shift+KeyZ"] },
  { id: "exportReader", label: "Export reader spreads", group: "Edit", keys: ["Ctrl+KeyE"] },
  { id: "exportPage", label: "Export single page", group: "Edit", keys: ["Ctrl+Shift+KeyE"] },
  { id: "shortcuts", label: "Shortcut keys", group: "Edit", keys: ["Ctrl+Slash"] },
  { id: "preferences", label: "Preferences", group: "Edit", keys: ["Ctrl+Comma"] },
  { id: "toggleGuides", label: "Show or hide guides", group: "View", keys: ["Ctrl+Semicolon"] },
  { id: "toggleCropMarks", label: "Show or hide crop marks", group: "View", keys: ["Ctrl+Shift+KeyM"] },
  { id: "toggleOutlines", label: "Show or hide frame outlines", group: "View", keys: ["Ctrl+Shift+KeyO"] },
  { id: "toggleBleed", label: "Show or hide bleed", group: "View", keys: ["Ctrl+Shift+KeyB"] },
  { id: "togglePages", label: "Show or hide pages", group: "View", keys: ["Ctrl+ArrowLeft"] },
  { id: "togglePanel", label: "Show or hide document panel", group: "View", keys: ["Ctrl+ArrowRight"] },
  { id: "zoomIn", label: "Zoom in", group: "View", keys: ["Ctrl+Equal"] },
  { id: "zoomOut", label: "Zoom out", group: "View", keys: ["Ctrl+Minus"] },
  { id: "fit", label: "Fit", group: "View", keys: ["Ctrl+Digit0"] },
  { id: "printPaged", label: "Print paged", group: "View", keys: ["Ctrl+KeyP"] },
  { id: "printViewed", label: "Print viewed", group: "View", keys: ["Ctrl+Shift+KeyP"] },
  { id: "printFolded", label: "Print folded", group: "View", keys: ["Ctrl+Alt+KeyP"] },
  { id: "toolSelect", label: "Select tool", group: "Tools", keys: ["KeyV"] },
  { id: "toolText", label: "Text tool", group: "Tools", keys: ["KeyT"] },
  { id: "toolImage", label: "Image tool", group: "Tools", keys: ["KeyI"] },
  { id: "toolLine", label: "Line tool", group: "Tools", keys: ["KeyL"] },
];

export const DEFAULT_SHORTCUTS: Record<CommandId, string[]> = Object.fromEntries(
  COMMANDS.map((command) => [command.id, command.keys]),
) as Record<CommandId, string[]>;

const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "ShiftLeft",
  "ShiftRight",
  "AltLeft",
  "AltRight",
  "MetaLeft",
  "MetaRight",
  "CapsLock",
]);

/** The shortcut a key press makes, such as "Ctrl+Shift+KeyZ"; null for a lone modifier. */
export function comboOf(event: KeyboardEvent): string | null {
  if (!event.code || MODIFIER_CODES.has(event.code)) return null;
  const parts: string[] = [];
  if (event.ctrlKey) parts.push("Ctrl");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  parts.push(event.code);
  return parts.join("+");
}

const KEY_NAMES: Record<string, string> = {
  ArrowLeft: "←",
  ArrowRight: "→",
  ArrowUp: "↑",
  ArrowDown: "↓",
  Equal: "=",
  Minus: "-",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
  Backslash: "\\",
  BracketLeft: "[",
  BracketRight: "]",
  Backquote: "`",
  Space: "Space",
  Enter: "Enter",
  Escape: "Esc",
  Backspace: "Backspace",
  Delete: "Delete",
  Tab: "Tab",
  Home: "Home",
  End: "End",
  PageUp: "Page Up",
  PageDown: "Page Down",
};

/** How a shortcut reads: "Ctrl+Shift+Z", "Ctrl+←". */
export function describe(combo: string): string {
  return combo
    .split("+")
    .map((part) => {
      if (part.startsWith("Key")) return part.slice(3);
      if (part.startsWith("Digit")) return part.slice(5);
      if (part.startsWith("Numpad")) return `Num ${part.slice(6)}`;
      return KEY_NAMES[part] ?? part;
    })
    .join("+");
}

/** Whether a shortcut has no Ctrl or Alt, so it would type a character. */
export function isPlain(combo: string): boolean {
  return !combo.startsWith("Ctrl+") && !combo.startsWith("Alt+");
}

/** Keys that edit text in a field and must stay with the field. */
export function belongsToTextField(combo: string): boolean {
  const key = combo.split("+").pop() ?? "";
  const ctrl = combo.startsWith("Ctrl+");
  if (["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End", "Backspace", "Delete"].includes(key)) return true;
  return ctrl && ["KeyA", "KeyC", "KeyV", "KeyX", "KeyZ", "KeyY"].includes(key);
}
