<script lang="ts">
  // Preferences: appearance, what shows at start, new-zine defaults,
  // printing resolution, and every shortcut key. Nothing changes until Save.

  import { COMMANDS, DEFAULT_SHORTCUTS, comboOf, describe, type CommandId } from "./commands";
  import { DEFAULT_PREFERENCES, prefs, type Preferences } from "./prefs.svelte";
  import { app } from "./state.svelte";
  import { open } from "@tauri-apps/plugin-dialog";

  type Tab = "general" | "new" | "shortcuts";

  let { tab = "general", onclose }: { tab?: Tab; onclose: () => void } = $props();

  // svelte-ignore state_referenced_locally
  let current = $state<Tab>(tab);
  let draft = $state<Preferences>(structuredClone($state.snapshot(prefs.current)) as Preferences);
  /** The command waiting for its new key, and which of its keys is being replaced (-1 adds one). */
  let recording = $state<{ id: CommandId; index: number } | null>(null);
  let notice = $state<string | null>(null);
  let saving = $state(false);

  const families = $derived(app.fontFamilies);
  const groups = $derived([...new Set(COMMANDS.map((command) => command.group))]);

  /** Chooses a picture for the desk; it is shown once the preferences are saved. */
  async function chooseDeskImage() {
    const chosen = await open({
      title: "Choose a picture for the desk",
      filters: [{ name: "Pictures", extensions: ["jpg", "jpeg", "png", "tif", "tiff"] }],
    });
    if (typeof chosen === "string") draft.desk_image = chosen;
  }

  function fileName(path: string) {
    return path.split(/[\\/]/).pop() ?? path;
  }

  function withCurrent(list: string[], value: string): string[] {
    return !value || list.includes(value) ? list : [value, ...list];
  }

  // While recording, the next key press (other than a lone modifier) becomes the shortcut.
  function onKeydownCapture(event: KeyboardEvent) {
    if (!recording) {
      if (event.key === "Escape") {
        event.stopPropagation();
        onclose();
      }
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Escape" && !event.ctrlKey && !event.altKey && !event.shiftKey) {
      recording = null;
      return;
    }
    const combo = comboOf(event);
    if (!combo) return;
    const { id, index } = recording;
    notice = null;
    // A shortcut does one thing: take it from any other command that had it.
    for (const other of COMMANDS) {
      const keys = draft.shortcuts[other.id];
      if (other.id !== id && keys.includes(combo)) {
        draft.shortcuts[other.id] = keys.filter((key) => key !== combo);
        notice = `${describe(combo)} was "${other.label}"; that command no longer has it.`;
      }
    }
    const keys = draft.shortcuts[id].filter((key) => key !== combo);
    if (index >= 0 && index < keys.length) keys[index] = combo;
    else keys.push(combo);
    draft.shortcuts[id] = keys;
    recording = null;
  }

  function removeKey(id: CommandId, combo: string) {
    draft.shortcuts[id] = draft.shortcuts[id].filter((key) => key !== combo);
  }

  async function save() {
    saving = true;
    try {
      await prefs.save($state.snapshot(draft) as Preferences);
      onclose();
    } catch (error) {
      notice = String(error);
    } finally {
      saving = false;
    }
  }
</script>

<svelte:window onkeydowncapture={onKeydownCapture} />

<div class="modal-backdrop" role="presentation">
  <div class="modal prefs" role="dialog" aria-modal="true" aria-label="Preferences">
    <h2>Preferences</h2>
    <div class="tabs" role="tablist">
      <button role="tab" aria-selected={current === "general"} class:active={current === "general"} onclick={() => (current = "general")}>General</button>
      <button role="tab" aria-selected={current === "new"} class:active={current === "new"} onclick={() => (current = "new")}>New zines</button>
      <button role="tab" aria-selected={current === "shortcuts"} class:active={current === "shortcuts"} onclick={() => (current = "shortcuts")}>Shortcut keys</button>
    </div>

    {#if current === "general"}
      <h3>Appearance</h3>
      <div class="grid">
        <label>
          Colour scheme
          <select bind:value={draft.theme}>
            <option value="system">Same as Windows</option>
            <option value="light">Lo Sport (light)</option>
            <option value="dark">Uomo di ferro (dark)</option>
          </select>
        </label>
        <label>
          Interface text size (px)
          <input type="number" min="10" max="20" step="1" bind:value={draft.ui_font_size} />
        </label>
        <label class="wide">
          Interface font
          <select bind:value={draft.ui_font}>
            <option value="">Windows default (Segoe UI)</option>
            {#each families as family}
              <option value={family}>{family}</option>
            {/each}
          </select>
        </label>
      </div>
      <div class="desk-choice">
        <span class="label">Desk behind the pages</span>
        <span class="value" title={draft.desk_image || undefined}>{draft.desk_image ? fileName(draft.desk_image) : "Gino's own"}</span>
        <button onclick={chooseDeskImage}>Choose picture…</button>
        <button onclick={() => (draft.desk_image = "")} disabled={!draft.desk_image}>Use Gino's own</button>
      </div>
      <div class="note">The page always stays white, as it will print. A chosen picture fills the desk in both colour schemes.</div>

      <h3>Korean</h3>
      <label class="check-row"><input type="checkbox" bind:checked={draft.show_korean} />Show the Korean options (font, scale, line breaking)</label>
      <div class="note">Korean text can always be typed; this only shows the controls for setting it.</div>

      <h3>Show when Gino starts</h3>
      <div class="grid">
        <label class="check-row"><input type="checkbox" bind:checked={draft.show_guides} />Guides (margins, columns)</label>
        <label class="check-row"><input type="checkbox" bind:checked={draft.show_crop_marks} />Crop marks at the text area</label>
        <label class="check-row"><input type="checkbox" bind:checked={draft.show_outlines} />Frame outlines</label>
        <label class="check-row"><input type="checkbox" bind:checked={draft.show_bleed} />Bleed</label>
        <label class="check-row"><input type="checkbox" bind:checked={draft.show_pages} />Pages</label>
        <label class="check-row"><input type="checkbox" bind:checked={draft.show_panel} />Document panel</label>
      </div>

      <h3>Printing and export</h3>
      <div class="grid">
        <label>
          Image resolution (dpi)
          <select bind:value={draft.print_dpi}>
            <option value={150}>150</option>
            <option value={300}>300</option>
            <option value={600}>600</option>
            <option value={0}>Keep every pixel</option>
          </select>
        </label>
      </div>
      <div class="note">Pictures finer than this are reduced when printing or exporting; coarser ones are left as they are.</div>
    {:else if current === "new"}
      <label class="check-row">
        <input type="checkbox" checked={!draft.starter_frames} onchange={(e) => (draft.starter_frames = !(e.target as HTMLInputElement).checked)} />
        Start new zines with no image or text frames
      </label>
      <div class="note">
        Off (the default): an image frame on the front and back covers, a blank inside front cover, and one text story
        threaded through page 3 to the second-to-last.
      </div>
      <h3>Body text</h3>
      <div class="grid">
        <label class="wide">
          Latin font
          <select bind:value={draft.new_latin_font}>
            {#each withCurrent(families, draft.new_latin_font) as family}
              <option value={family}>{family}</option>
            {/each}
          </select>
        </label>
        {#if draft.show_korean}
          <label class="wide">
            Korean font
            <select bind:value={draft.new_korean_font}>
              {#each withCurrent(families, draft.new_korean_font) as family}
                <option value={family}>{family}</option>
              {/each}
            </select>
          </label>
        {/if}
        <label>Size (pt)<input type="number" min="4" step="0.5" bind:value={draft.new_size} /></label>
        <label>Leading (pt)<input type="number" min="4" step="0.5" bind:value={draft.new_leading} /></label>
      </div>
    {:else}
      <div class="note" style="margin-top: 0;">
        Click a shortcut to change it, then press the new keys. + adds another. Esc cancels.
      </div>
      <table class="shortcut-table">
        <tbody>
          {#each groups as group}
            <tr><th colspan="2">{group}</th></tr>
            {#each COMMANDS.filter((command) => command.group === group) as command (command.id)}
              <tr>
                <td>{command.label}</td>
                <td class="keys">
                  {#each draft.shortcuts[command.id] as combo, index (combo)}
                    <span class="key">
                      <button
                        class:recording={recording?.id === command.id && recording.index === index}
                        onclick={() => (recording = { id: command.id, index })}
                        title="Change">{recording?.id === command.id && recording.index === index ? "Press keys…" : describe(combo)}</button
                      ><button class="remove" title="Remove" aria-label={`Remove ${describe(combo)}`} onclick={() => removeKey(command.id, combo)}>×</button>
                    </span>
                  {/each}
                  <button
                    class="add"
                    class:recording={recording?.id === command.id && recording.index === -1}
                    onclick={() => (recording = { id: command.id, index: -1 })}
                    title="Add a shortcut">{recording?.id === command.id && recording.index === -1 ? "Press keys…" : "+"}</button
                  >
                </td>
              </tr>
            {/each}
          {/each}
          <tr><th colspan="2">On the canvas (fixed)</th></tr>
          <tr><td>Move the selected frame</td><td class="keys">Arrows; Shift+arrows by 5 mm</td></tr>
          <tr><td>Delete the selected frame</td><td class="keys">Delete</td></tr>
          <tr><td>Type in the selected text frame</td><td class="keys">Enter, or double-click</td></tr>
          <tr><td>Previous / next spread</td><td class="keys">Page Up / Page Down</td></tr>
          <tr><td>Stop typing, cancel, deselect</td><td class="keys">Esc</td></tr>
          <tr><td>While typing</td><td class="keys">Ctrl+arrows by word; Shift selects; Ctrl+A, Ctrl+Backspace</td></tr>
        </tbody>
      </table>
      <div class="row-actions">
        <button onclick={() => (draft.shortcuts = structuredClone(DEFAULT_SHORTCUTS))}>Reset all shortcuts</button>
      </div>
    {/if}

    {#if notice}
      <div class="note" style="color: var(--warn);">{notice}</div>
    {/if}
    <div class="actions">
      <button onclick={() => (draft = structuredClone(DEFAULT_PREFERENCES))} title="Every preference back to how Gino came">Defaults</button>
      <span style="flex: 1"></span>
      <button onclick={onclose}>Cancel</button>
      <button onclick={save} disabled={saving}>Save</button>
    </div>
  </div>
</div>
