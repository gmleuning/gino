<script lang="ts">
  import { onMount } from "svelte";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { api, type Size, type Tool } from "./lib/api";
  import { app, run } from "./lib/state.svelte";
  import { prefs, watchSystemTheme } from "./lib/prefs.svelte";
  import { belongsToTextField, comboOf, describe, isPlain, type CommandId } from "./lib/commands";
  import PageStrip from "./lib/PageStrip.svelte";
  import Canvas from "./lib/Canvas.svelte";
  import Inspector from "./lib/Inspector.svelte";
  import NewDocument from "./lib/NewDocument.svelte";
  import PreferencesDialog from "./lib/PreferencesDialog.svelte";
  import MenuBar, { type Menu, type MenuItem } from "./lib/MenuBar.svelte";

  type PrintMode = "paged" | "viewed" | "folded";

  let canvas: Canvas | undefined = $state();
  let showNew = $state(false);
  let showPrint = $state(false);
  let preferencesTab = $state<"general" | "new" | "shortcuts" | null>(null);
  let printMode = $state<PrintMode>("paged");
  let printing = $state(false);
  let printSheet = $state<"document" | "a3" | "a4" | "custom">("document");
  let customSheet = $state<Size>({ width: 320, height: 450 });
  let printDpi = $state(300);
  let showExportPage = $state(false);
  let exportPageNumber = $state(1);

  const tools: { tool: Tool; label: string; command: CommandId }[] = [
    { tool: "select", label: "Select", command: "toolSelect" },
    { tool: "text", label: "Text", command: "toolText" },
    { tool: "image", label: "Image", command: "toolImage" },
    { tool: "rule", label: "Line", command: "toolLine" },
  ];

  onMount(async () => {
    const loaded = await prefs.load();
    watchSystemTheme();
    app.showGuides = loaded.show_guides;
    app.showCropMarks = loaded.show_crop_marks;
    app.showOutlines = loaded.show_outlines;
    app.showBleed = loaded.show_bleed;
    app.showPages = loaded.show_pages;
    app.showPanel = loaded.show_panel;
    loadFonts();
  });

  async function loadFonts() {
    try {
      app.fontFamilies = await api.fontFamilies();
    } catch (error) {
      app.say(String(error));
    }
  }

  function opened() {
    loadFonts();
    setTimeout(() => {
      canvas?.fit();
      canvas?.focus();
    }, 0);
  }

  async function openDocument(path?: string) {
    try {
      const chosen =
        path ??
        (await open({
          directory: true,
          title: "Open a .zine folder",
        }));
      if (!chosen) return;
      const view = await api.openDocument(chosen as string);
      app.currentSpread = 0;
      app.openView(view);
      app.say(`Opened ${view.title}`);
      opened();
    } catch (error) {
      app.say(String(error));
    }
  }

  function overflowWarning(overflows: { story: string; characters: number }[]) {
    return overflows.length > 0
      ? ` Overflow: ${overflows.map((o) => `${o.story} (${o.characters} characters)`).join(", ")}.`
      : "";
  }

  /** The preference's resolution, or null to keep every pixel. */
  const preferredDpi = () => (prefs.current.print_dpi > 0 ? prefs.current.print_dpi : null);

  async function exportReader() {
    if (!app.view) return;
    try {
      const suggested = await api.defaultExportPath("reader");
      const target = await save({
        title: "Export reader spreads",
        defaultPath: suggested,
        filters: [{ name: "PDF", extensions: ["pdf"] }],
      });
      if (!target) return;
      const result = await api.exportReader(target, preferredDpi());
      app.say(`Exported ${result.path}.${overflowWarning(result.overflows)}`);
    } catch (error) {
      app.say(String(error));
    }
  }

  /** The page shown first in the current spread, or the selected frame's page. */
  function currentPage(): number {
    if (app.selection) return app.selection.page;
    return app.view?.spreads[app.currentSpread]?.pages[0]?.[0] ?? 0;
  }

  function openExportPage() {
    if (!app.view) return;
    exportPageNumber = currentPage() + 1;
    showExportPage = true;
  }

  async function exportPage() {
    try {
      const page = Math.round(exportPageNumber) - 1;
      const suggested = await api.defaultExportPath(`page-${page + 1}`);
      const target = await save({
        title: `Export page ${page + 1}`,
        defaultPath: suggested,
        filters: [{ name: "PDF", extensions: ["pdf"] }],
      });
      if (!target) return;
      showExportPage = false;
      const result = await api.exportPage(target, page, preferredDpi());
      app.say(`Exported ${result.path}.`);
    } catch (error) {
      app.say(String(error));
    }
  }

  function openPrint(mode: PrintMode) {
    if (!app.view) return;
    printMode = mode;
    printDpi = prefs.current.print_dpi;
    showPrint = true;
  }

  const saveDocument = () => {
    if (app.view) run(api.save).then((update) => update && app.say("Saved"));
  };

  function setTool(tool: Tool) {
    app.tool = tool;
    canvas?.focus();
  }

  /** What each command does. The menus and the shortcut keys both run these. */
  const actions: Record<CommandId, () => void> = {
    new: () => (showNew = true),
    open: () => openDocument(),
    save: saveDocument,
    undo: () => app.view && run(api.undo),
    redo: () => app.view && run(api.redo),
    exportReader,
    exportPage: openExportPage,
    shortcuts: () => (preferencesTab = "shortcuts"),
    preferences: () => (preferencesTab = "general"),
    toggleGuides: () => (app.showGuides = !app.showGuides),
    toggleCropMarks: () => (app.showCropMarks = !app.showCropMarks),
    toggleOutlines: () => (app.showOutlines = !app.showOutlines),
    toggleBleed: () => (app.showBleed = !app.showBleed),
    togglePages: () => (app.showPages = !app.showPages),
    togglePanel: () => (app.showPanel = !app.showPanel),
    zoomIn: () => canvas?.zoomBy(1.25),
    zoomOut: () => canvas?.zoomBy(1 / 1.25),
    fit: () => canvas?.fit(),
    printPaged: () => openPrint("paged"),
    printViewed: () => openPrint("viewed"),
    printFolded: () => openPrint("folded"),
    toolSelect: () => setTool("select"),
    toolText: () => setTool("text"),
    toolImage: () => setTool("image"),
    toolLine: () => setTool("rule"),
  };

  /** A menu item for a command, showing its first shortcut. */
  function item(id: CommandId, label: string, extra: Partial<MenuItem> = {}): MenuItem {
    const keys = prefs.current.shortcuts[id] ?? [];
    return { label, shortcut: keys[0] ? describe(keys[0]) : undefined, action: actions[id], ...extra };
  }

  const menus: Menu[] = $derived([
    {
      label: "File",
      items: [item("new", "New…"), item("open", "Open…"), item("save", "Save", { disabled: !app.view })],
    },
    {
      label: "Edit",
      items: [
        item("save", "Save", { disabled: !app.view }),
        { separator: true },
        item("undo", "Undo", { disabled: !app.canUndo }),
        item("redo", "Redo", { disabled: !app.canRedo }),
        { separator: true },
        {
          label: "Export",
          disabled: !app.view,
          submenu: [item("exportReader", "Reader spreads…"), item("exportPage", "Single page…")],
        },
        { separator: true },
        item("shortcuts", "Shortcut keys…"),
        item("preferences", "Preferences…"),
      ],
    },
    {
      label: "View",
      items: [
        item("toggleGuides", "Guides", { checked: app.showGuides }),
        item("toggleCropMarks", "Crop marks", { checked: app.showCropMarks }),
        item("toggleOutlines", "Frame outlines", { checked: app.showOutlines }),
        item("toggleBleed", "Bleed", { checked: app.showBleed }),
        { separator: true },
        item("togglePages", "Pages", { checked: app.showPages }),
        item("togglePanel", "Document panel", { checked: app.showPanel }),
        { separator: true },
        item("zoomIn", "Zoom in", { disabled: !app.view }),
        item("zoomOut", "Zoom out", { disabled: !app.view }),
        item("fit", "Fit", { disabled: !app.view }),
        { separator: true },
        {
          label: "Print",
          disabled: !app.view,
          submenu: [item("printPaged", "Paged…"), item("printViewed", "Viewed…"), item("printFolded", "Folded…")],
        },
      ],
    },
  ]);

  function chosenPrintSheet(): Size | null {
    switch (printSheet) {
      case "document":
        return null;
      case "a3":
        return { width: 297, height: 420 };
      case "a4":
        return { width: 210, height: 297 };
      case "custom":
        return { ...customSheet };
    }
  }

  /** Writes the chosen print PDF into the .zine folder and opens it in the PDF viewer to print. */
  async function print() {
    printing = true;
    try {
      const dpi = printDpi > 0 ? printDpi : null;
      const path = await api.defaultExportPath(`print-${printMode}`);
      const result =
        printMode === "paged"
          ? await api.exportPaged(path, dpi)
          : printMode === "viewed"
            ? await api.exportReader(path, dpi)
            : await api.exportImposed(path, chosenPrintSheet(), dpi);
      showPrint = false;
      await api.openInDefaultApp(result.path);
      const how = printMode === "folded" ? " Print double-sided, flipping on the short edge." : "";
      app.say(`Opened ${result.path} to print.${how}${overflowWarning(result.overflows)}`);
    } catch (error) {
      app.say(String(error));
    } finally {
      printing = false;
    }
  }

  const anyDialog = $derived(showNew || showPrint || showExportPage || preferencesTab !== null);

  /**
   * Shortcut keys. A key a text field or the text being typed has already
   * used (the canvas marks those as handled) is left alone; so are keys that
   * edit text inside a field, and single keys while typing.
   */
  function onKeydown(event: KeyboardEvent) {
    if (event.defaultPrevented || event.repeat && !event.ctrlKey) return;
    const combo = comboOf(event);
    if (!combo) return;
    const id = prefs.commandFor(combo);
    if (!id) return;
    const target = event.target as HTMLElement;
    const inField = !!target?.matches?.("input, textarea, select, [contenteditable]");
    if (inField && (isPlain(combo) || belongsToTextField(combo))) return;
    if (app.text && isPlain(combo)) return;
    if (anyDialog && id !== "new" && id !== "open") return;
    event.preventDefault();
    actions[id]();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div
  class="layout"
  style:grid-template-columns={`${app.showPages ? "180px " : ""}1fr${app.showPanel ? " auto" : ""}`}
>
  <div class="topbar">
    <div class="topbar-left"><MenuBar {menus} /></div>
    {#if app.view}
      <div class="tools" role="toolbar" aria-label="Tools">
        {#each tools as entry}
          {@const key = prefs.current.shortcuts[entry.command]?.[0]}
          <button
            class:active={app.tool === entry.tool}
            title={key ? `${entry.label} (${describe(key)})` : entry.label}
            onclick={() => setTool(entry.tool)}>{entry.label}</button
          >
        {/each}
      </div>
    {/if}
  </div>

  {#if app.showPages}
    <aside class="strip">
      <PageStrip />
    </aside>
  {/if}

  <main class="canvas-wrap" style="position: relative;">
    {#if app.view}
      <Canvas bind:this={canvas} />
    {:else}
      <div class="empty">
        <div>
          <p>Make a new zine, or open a <code>.zine</code> folder.</p>
          <button onclick={() => (showNew = true)}>New…</button>
          <button onclick={() => openDocument()}>Open…</button>
        </div>
      </div>
    {/if}
  </main>

  {#if app.showPanel}
    <aside class="inspector">
      <Inspector />
    </aside>
  {/if}

  <div class="statusbar">
    <span>{app.message ?? ""}</span>
    {#each app.overflows as overflow}
      <span class="warn">Overflow: story "{overflow.story}" has {overflow.characters} characters that do not fit (page {overflow.page + 1}).</span>
    {/each}
    {#each app.imageProblems as problem}
      <span class="warn">Page {problem.page + 1}, frame "{problem.frame}": {problem.message}</span>
    {/each}
    <span class="spacer" style="flex: 1"></span>
    {#if app.view}
      <span>{Math.round(app.zoom * 25.4)} px/in</span>
    {/if}
  </div>
</div>

{#if showNew}
  <NewDocument onclose={() => (showNew = false)} onopened={opened} />
{/if}

{#if showPrint && app.view}
  <div class="modal-backdrop" role="presentation">
    <div class="modal" role="dialog" aria-modal="true" aria-label="Print">
      <h2>Print</h2>
      <label class="choice">
        <input type="radio" bind:group={printMode} value="paged" />
        <span>Paged<small>Every page on its own sheet at page size ({app.view.settings.trim.width} × {app.view.settings.trim.height} mm), in reading order.</small></span>
      </label>
      <label class="choice">
        <input type="radio" bind:group={printMode} value="viewed" />
        <span>Viewed<small>As laid out in Gino: the covers alone, the inside pages as spreads.</small></span>
      </label>
      <label class="choice">
        <input type="radio" bind:group={printMode} value="folded" />
        <span>Folded<small>Pages in folding order, to print double-sided (flip on the short edge), fold and staple.</small></span>
      </label>
      <div class="grid" style="margin-top: 6px;">
        <label>
          Image resolution
          <select bind:value={printDpi}>
            <option value={150}>150 dpi</option>
            <option value={300}>300 dpi</option>
            <option value={600}>600 dpi</option>
            <option value={0}>Keep every pixel</option>
          </select>
        </label>
        {#if printMode === "folded"}
          <label class="wide">
            Sheet
            <select bind:value={printSheet}>
              <option value="document">
                The document's sheet ({app.view.settings.sheet.width} × {app.view.settings.sheet.height} mm)
              </option>
              <option value="a3">A3</option>
              <option value="a4">A4</option>
              <option value="custom">Another size…</option>
            </select>
          </label>
          {#if printSheet === "custom"}
            <label>Width (mm)<input type="number" min="50" step="1" bind:value={customSheet.width} /></label>
            <label>Height (mm)<input type="number" min="50" step="1" bind:value={customSheet.height} /></label>
          {/if}
        {/if}
      </div>
      <div class="note">Pictures finer than the resolution are reduced to it; coarser ones are left as they are. Text and lines are always sharp.</div>
      {#if printMode === "folded"}
        <div class="note">A sheet larger than the imposed pages gets bleed, crop marks and creep; a same-size sheet gets neither.</div>
      {/if}
      <div class="note">The PDF is saved in the .zine folder and opens in your PDF viewer, where you print it.</div>
      <div class="actions">
        <button onclick={() => (showPrint = false)}>Cancel</button>
        <button onclick={print} disabled={printing}>{printing ? "Preparing…" : "Print…"}</button>
      </div>
    </div>
  </div>
{/if}

{#if preferencesTab}
  <PreferencesDialog tab={preferencesTab} onclose={() => (preferencesTab = null)} />
{/if}

{#if showExportPage && app.view}
  <div class="modal-backdrop" role="presentation">
    <div class="modal" role="dialog" aria-modal="true" aria-label="Export a single page">
      <h2>Export a single page</h2>
      <div class="grid">
        <label>
          Page
          <input type="number" min="1" max={app.view.pages.length} step="1" bind:value={exportPageNumber} />
        </label>
      </div>
      <div class="note">A one-page PDF at page size ({app.view.settings.trim.width} × {app.view.settings.trim.height} mm).</div>
      <div class="actions">
        <button onclick={() => (showExportPage = false)}>Cancel</button>
        <button onclick={exportPage} disabled={exportPageNumber < 1 || exportPageNumber > app.view.pages.length}>Choose where to save…</button>
      </div>
    </div>
  </div>
{/if}
