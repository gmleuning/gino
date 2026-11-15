<script lang="ts">
  // The new-document dialog: a fold preset, page count, sheet, margins,
  // columns and the body text's fonts. The core fills in everything else.

  import { save } from "@tauri-apps/plugin-dialog";
  import { api, type Fold, type Margins, type NewDocument, type Size } from "./api";
  import { app } from "./state.svelte";
  import { prefs } from "./prefs.svelte";

  let { onclose, onopened }: { onclose: () => void; onopened: () => void } = $props();

  type SheetChoice = "same" | "a3" | "a4" | "custom";

  let fold = $state<Fold>("saddle_a5_on_a4");
  let pageCount = $state(16);
  let customTrim = $state<Size>({ width: 140, height: 216 });
  let sheetChoice = $state<SheetChoice>("same");
  let customSheet = $state<Size>({ width: 320, height: 450 });
  let margins = $state<Margins>({ top: 12, bottom: 15, inside: 12, outside: 10 });
  let columns = $state(1);
  let gutter = $state(4);
  let latinFont = $state(prefs.current.new_latin_font);
  let koreanFont = $state(prefs.current.new_korean_font);
  let size = $state(prefs.current.new_size);
  let leading = $state(prefs.current.new_leading);
  let defaults = $state<{ trim: Size; sheet: Size } | null>(null);
  let starterFrames = $state(prefs.current.starter_frames);
  let busy = $state(false);

  let error = $state<string | null>(null);

  const folds: { value: Fold; label: string }[] = [
    { value: "saddle_a5_on_a4", label: "Saddle stitch: A5 pages, printed on A4" },
    { value: "saddle_a6_on_a5", label: "Saddle stitch: A6 pages, printed on A5" },
    { value: "eight_page_fold", label: "Eight-page fold: one A4 sheet, cut and folded" },
    { value: "custom", label: "Saddle stitch: custom page size" },
  ];

  // Fill the margins and sizes from the fold whenever it (or the custom trim) changes.
  $effect(() => {
    const chosen = fold;
    const trim = chosen === "custom" ? { ...customTrim } : null;
    api
      .foldDefaults(chosen, trim)
      .then((result) => {
        defaults = { trim: result.trim, sheet: result.sheet };
        margins = result.margins;
      })
      .catch((reason) => (error = String(reason)));
  });

  $effect(() => {
    if (fold === "eight_page_fold") pageCount = 8;
  });

  $effect(() => {
    if (app.fontFamilies.length === 0) {
      api.fontFamilies().then((families) => (app.fontFamilies = families));
    }
  });

  const families = $derived(app.fontFamilies);

  function sheet(): Size | null {
    switch (sheetChoice) {
      case "same":
        return null;
      case "a3":
        return { width: 297, height: 420 };
      case "a4":
        return { width: 210, height: 297 };
      case "custom":
        return { ...customSheet };
    }
  }

  async function create() {
    error = null;
    const request: NewDocument = {
      fold,
      custom_trim: fold === "custom" ? { ...customTrim } : null,
      page_count: fold === "eight_page_fold" ? 8 : pageCount,
      sheet: sheet(),
      margins: { ...margins },
      columns: { count: columns, gutter },
      latin_font: latinFont,
      korean_font: koreanFont,
      size,
      leading,
      starter_frames: starterFrames,
    };
    const target = await save({
      title: "Where to keep the new zine (a .zine folder is made there)",
      defaultPath: "untitled.zine",
    });
    if (!target) return;
    busy = true;
    try {
      const view = await api.newDocument(target, request);
      app.openView(view);
      app.say(`Made ${view.folder}`);
      onopened();
      onclose();
    } catch (reason) {
      error = String(reason);
    } finally {
      busy = false;
    }
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") onclose();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="modal-backdrop" role="presentation">
  <div class="modal" role="dialog" aria-modal="true" aria-label="New zine">
    <h2>New zine</h2>
    <div class="grid">
      <label class="wide">
        Fold
        <select bind:value={fold}>
          {#each folds as option}
            <option value={option.value}>{option.label}</option>
          {/each}
        </select>
      </label>
      {#if fold === "custom"}
        <label>Page width (mm)<input type="number" min="30" step="1" bind:value={customTrim.width} /></label>
        <label>Page height (mm)<input type="number" min="30" step="1" bind:value={customTrim.height} /></label>
      {/if}
      {#if defaults}
        <div class="wide note" style="margin-top: 0;">
          Pages {defaults.trim.width} × {defaults.trim.height} mm; same-size sheet {defaults.sheet.width} × {defaults.sheet.height} mm.
        </div>
      {/if}
      <label>
        Pages
        <input type="number" min="4" step={fold === "eight_page_fold" ? 8 : 4} bind:value={pageCount} disabled={fold === "eight_page_fold"} />
      </label>
      <label>
        Print on
        <select bind:value={sheetChoice}>
          <option value="same">The fold's own sheet (no bleed)</option>
          <option value="a3">A3, then trim (bleed and crop marks)</option>
          <option value="a4">A4, then trim</option>
          <option value="custom">Another sheet size…</option>
        </select>
      </label>
      {#if sheetChoice === "custom"}
        <label>Sheet width (mm)<input type="number" min="50" step="1" bind:value={customSheet.width} /></label>
        <label>Sheet height (mm)<input type="number" min="50" step="1" bind:value={customSheet.height} /></label>
      {/if}

      <label>Top margin (mm)<input type="number" min="0" step="0.5" bind:value={margins.top} /></label>
      <label>Bottom margin (mm)<input type="number" min="0" step="0.5" bind:value={margins.bottom} /></label>
      <label>Inside margin (mm)<input type="number" min="0" step="0.5" bind:value={margins.inside} /></label>
      <label>Outside margin (mm)<input type="number" min="0" step="0.5" bind:value={margins.outside} /></label>
      <label>Columns<input type="number" min="1" max="6" step="1" bind:value={columns} /></label>
      <label>Gutter (mm)<input type="number" min="0" step="0.5" bind:value={gutter} /></label>

      <label class="wide">
        Latin font
        <select bind:value={latinFont}>
          {#each families.includes(latinFont) ? families : [latinFont, ...families] as family}
            <option value={family}>{family}</option>
          {/each}
        </select>
      </label>
      <label class="wide">
        Korean font
        <select bind:value={koreanFont}>
          {#each families.includes(koreanFont) ? families : [koreanFont, ...families] as family}
            <option value={family}>{family}</option>
          {/each}
        </select>
      </label>
      <label>Size (pt)<input type="number" min="4" step="0.5" bind:value={size} /></label>
      <label>Leading (pt)<input type="number" min="4" step="0.5" bind:value={leading} /></label>
    </div>
    <label class="check-row">
      <input type="checkbox" bind:checked={starterFrames} />
      Start with cover image frames and a text story through pages 3 to {fold === "eight_page_fold" ? 7 : "the second-to-last"}
    </label>
    <div class="note">The page count is rounded up to a multiple of four. Fold and page size cannot change later.</div>
    {#if error}
      <div class="note" style="color: var(--warn);">{error}</div>
    {/if}
    <div class="actions">
      <button onclick={onclose}>Cancel</button>
      <button onclick={create} disabled={busy}>Choose where to save…</button>
    </div>
  </div>
</div>
