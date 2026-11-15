<script lang="ts">
  // Document settings: margins, columns, sheet, bleed, printer margin,
  // creep and the page count. The fold and trim size are fixed once a
  // document is made.

  import { api, type Settings } from "./api";
  import { app, run } from "./state.svelte";

  const settings = $derived(app.view?.settings ?? null);

  const foldName: Record<string, string> = {
    saddle_a5_on_a4: "Saddle stitch, A5 pages on A4",
    saddle_a6_on_a5: "Saddle stitch, A6 pages on A5",
    eight_page_fold: "Eight-page fold on one A4 sheet",
    custom: "Saddle stitch, custom size",
  };

  function number(event: Event): number | null {
    const value = Number((event.target as HTMLInputElement).value);
    return Number.isFinite(value) ? value : null;
  }

  function change(edit: (settings: Settings) => void) {
    if (!settings) return;
    const next: Settings = structuredClone($state.snapshot(settings)) as Settings;
    edit(next);
    run(() => api.setSettings(next));
  }

  const pageStep = $derived(settings?.fold === "eight_page_fold" ? 8 : 4);
</script>

{#if app.view && settings}
  <div class="doc-title">{app.view.title}{app.dirty ? " •" : ""}</div>
  <div class="path">{app.view.folder}</div>
  <div class="path">
    {foldName[settings.fold] ?? settings.fold} · {settings.trim.width} × {settings.trim.height} mm pages ·
    {app.view.pages.length} pages
  </div>
  <div class="path">
    Sheet {settings.sheet.width} × {settings.sheet.height} mm,
    {app.view.sheet_mode === "oversize" ? "oversize: bleed and crop marks" : "same size: no bleed"}
  </div>

  <section>
    <h2>Show</h2>
    <div class="grid">
      <label class="check"><input type="checkbox" bind:checked={app.showGuides} />Guides</label>
      <label class="check"><input type="checkbox" bind:checked={app.showCropMarks} />Crop marks</label>
      <label class="check"><input type="checkbox" bind:checked={app.showOutlines} />Frame outlines</label>
      <label class="check"><input type="checkbox" bind:checked={app.showBleed} />Bleed</label>
    </div>
  </section>

  <section>
    <h2>Pages</h2>
    <div class="row" style="margin-top: 0;">
      <button onclick={() => run(() => api.addPages(pageStep))} disabled={settings.fold === "eight_page_fold"}>Add {pageStep}</button>
      <button onclick={() => run(() => api.removePages(pageStep))} disabled={settings.fold === "eight_page_fold" || app.view.pages.length <= pageStep}>
        Remove last {pageStep}
      </button>
    </div>
    {#if settings.fold === "eight_page_fold"}
      <div class="path">An eight-page fold always has eight pages.</div>
    {/if}
  </section>

  <section>
    <h2>Margins (mm)</h2>
    <div class="grid">
      <label>Top<input type="number" step="0.5" min="0" value={settings.margins.top} onchange={(e) => { const v = number(e); if (v !== null) change((s) => (s.margins.top = v)); }} /></label>
      <label>Bottom<input type="number" step="0.5" min="0" value={settings.margins.bottom} onchange={(e) => { const v = number(e); if (v !== null) change((s) => (s.margins.bottom = v)); }} /></label>
      <label>Inside<input type="number" step="0.5" min="0" value={settings.margins.inside} onchange={(e) => { const v = number(e); if (v !== null) change((s) => (s.margins.inside = v)); }} /></label>
      <label>Outside<input type="number" step="0.5" min="0" value={settings.margins.outside} onchange={(e) => { const v = number(e); if (v !== null) change((s) => (s.margins.outside = v)); }} /></label>
      <label>Columns<input type="number" step="1" min="1" max="6" value={settings.columns.count} onchange={(e) => { const v = number(e); if (v !== null) change((s) => (s.columns.count = Math.max(1, Math.round(v)))); }} /></label>
      <label>Gutter<input type="number" step="0.5" min="0" value={settings.columns.gutter} onchange={(e) => { const v = number(e); if (v !== null) change((s) => (s.columns.gutter = v)); }} /></label>
    </div>
  </section>

  <section>
    <h2>Printing (mm)</h2>
    <div class="grid">
      <label>Sheet width<input type="number" step="1" min="10" value={settings.sheet.width} onchange={(e) => { const v = number(e); if (v !== null) change((s) => (s.sheet.width = v)); }} /></label>
      <label>Sheet height<input type="number" step="1" min="10" value={settings.sheet.height} onchange={(e) => { const v = number(e); if (v !== null) change((s) => (s.sheet.height = v)); }} /></label>
      <label>Bleed<input type="number" step="0.5" min="0" value={settings.bleed} onchange={(e) => { const v = number(e); if (v !== null) change((s) => (s.bleed = v)); }} /></label>
      <label>Printer margin<input type="number" step="0.5" min="0" value={settings.printer_margin} onchange={(e) => { const v = number(e); if (v !== null) change((s) => (s.printer_margin = v)); }} /></label>
      <label>Creep per sheet<input type="number" step="0.05" min="0" value={settings.creep_per_sheet} onchange={(e) => { const v = number(e); if (v !== null) change((s) => (s.creep_per_sheet = v)); }} /></label>
    </div>
    <div class="path">Bleed and creep apply only on an oversize sheet. The printer margin is drawn as a guide.</div>
  </section>

  <div class="row" style="font-size: 11px; color: var(--muted);">
    Tools: V select, T text, I image, L line. Click a frame to select it and drag to move it; double-click a text frame to type.
  </div>
{:else}
  <h2>Gino</h2>
  <div class="path">No document open.</div>
{/if}
