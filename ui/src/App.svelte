<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { api, type Size, type Tool } from "./lib/api";
  import { app, run } from "./lib/state.svelte";
  import PageStrip from "./lib/PageStrip.svelte";
  import Canvas from "./lib/Canvas.svelte";
  import Inspector from "./lib/Inspector.svelte";
  import NewDocument from "./lib/NewDocument.svelte";

  let canvas: Canvas | undefined = $state();
  let showNew = $state(false);
  let showPrint = $state(false);
  let showPreferences = $state(false);
  let starterFrames = $state(true);
  let printMode = $state<"paged" | "viewed" | "folded">("paged");
  let printing = $state(false);
  let printSheet = $state<"document" | "a3" | "a4" | "custom">("document");
  let customSheet = $state<Size>({ width: 320, height: 450 });

  const tools: { tool: Tool; label: string; key: string }[] = [
    { tool: "select", label: "Select", key: "V" },
    { tool: "text", label: "Text", key: "T" },
    { tool: "image", label: "Image", key: "I" },
    { tool: "rule", label: "Line", key: "L" },
  ];

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

  async function exportReader() {
    try {
      const suggested = await api.defaultExportPath("reader");
      const target = await save({
        title: "Export reader spreads",
        defaultPath: suggested,
        filters: [{ name: "PDF", extensions: ["pdf"] }],
      });
      if (!target) return;
      const result = await api.exportReader(target);
      app.say(`Exported ${result.path}.${overflowWarning(result.overflows)}`);
    } catch (error) {
      app.say(String(error));
    }
  }

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
      const path = await api.defaultExportPath(`print-${printMode}`);
      const result =
        printMode === "paged"
          ? await api.exportPaged(path)
          : printMode === "viewed"
            ? await api.exportReader(path)
            : await api.exportImposed(path, chosenPrintSheet());
      showPrint = false;
      await api.openInDefaultApp(result.path);
      const how =
        printMode === "folded" ? " Print double-sided, flipping on the short edge." : "";
      app.say(`Opened ${result.path} to print.${how}${overflowWarning(result.overflows)}`);
    } catch (error) {
      app.say(String(error));
    } finally {
      printing = false;
    }
  }

  async function openPreferences() {
    try {
      starterFrames = (await api.getPreferences()).starter_frames;
    } catch {
      starterFrames = true;
    }
    showPreferences = true;
  }

  async function savePreferences() {
    try {
      await api.setPreferences({ starter_frames: starterFrames });
      showPreferences = false;
    } catch (error) {
      app.say(String(error));
    }
  }

  function onKeydown(event: KeyboardEvent) {
    const target = event.target as HTMLElement;
    const inField = target?.matches?.("input, textarea, select, [contenteditable]");
    const key = event.key.toLowerCase();
    if (event.ctrlKey && key === "o") {
      event.preventDefault();
      openDocument();
    } else if (event.ctrlKey && key === "n") {
      event.preventDefault();
      showNew = true;
    } else if (event.ctrlKey && key === "s" && app.view) {
      event.preventDefault();
      run(api.save).then(() => app.say("Saved"));
    } else if (event.ctrlKey && key === "z" && app.view && !inField) {
      event.preventDefault();
      run(event.shiftKey ? api.redo : api.undo);
    } else if (event.ctrlKey && key === "y" && app.view && !inField) {
      event.preventDefault();
      run(api.redo);
    } else if (event.ctrlKey && key === "e" && app.view) {
      event.preventDefault();
      exportReader();
    } else if (event.ctrlKey && key === "p" && app.view) {
      event.preventDefault();
      showPrint = true;
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="layout">
  <div class="topbar">
    <button onclick={() => (showNew = true)}>New…</button>
    <button onclick={() => openDocument()}>Open…</button>
    <button disabled={!app.view || !app.dirty} onclick={() => run(api.save).then(() => app.say("Saved"))}>Save</button>
    <button disabled={!app.canUndo} onclick={() => run(api.undo)}>Undo</button>
    <button disabled={!app.canRedo} onclick={() => run(api.redo)}>Redo</button>
    {#if app.view}
      <div class="tools" role="toolbar" aria-label="Tools">
        {#each tools as entry}
          <button
            class:active={app.tool === entry.tool}
            title={`${entry.label} (${entry.key})`}
            onclick={() => {
              app.tool = entry.tool;
              canvas?.focus();
            }}>{entry.label}</button
          >
        {/each}
      </div>
    {/if}
    <span class="spacer"></span>
    <button disabled={!app.view} onclick={exportReader}>Export reader spreads…</button>
    <button disabled={!app.view} onclick={() => (showPrint = true)} title="Ctrl+P">Print…</button>
    <button onclick={openPreferences}>Preferences…</button>
    <button disabled={!app.view} onclick={() => canvas?.zoomBy(1 / 1.25)}>−</button>
    <button disabled={!app.view} onclick={() => canvas?.fit()}>Fit</button>
    <button disabled={!app.view} onclick={() => canvas?.zoomBy(1.25)}>+</button>
  </div>

  <aside class="strip">
    <PageStrip />
  </aside>

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

  <aside class="inspector">
    <Inspector />
  </aside>

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
        <span>Viewed<small>As laid out in Gino's: the covers alone, the inside pages as spreads.</small></span>
      </label>
      <label class="choice">
        <input type="radio" bind:group={printMode} value="folded" />
        <span>Folded<small>Pages in folding order, to print double-sided (flip on the short edge), fold and staple.</small></span>
      </label>
      {#if printMode === "folded"}
        <div class="grid" style="margin-top: 6px;">
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
        </div>
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

{#if showPreferences}
  <div class="modal-backdrop" role="presentation">
    <div class="modal" role="dialog" aria-modal="true" aria-label="Preferences">
      <h2>Preferences</h2>
      <label class="check-row">
        <input type="checkbox" checked={!starterFrames} onchange={(e) => (starterFrames = !(e.target as HTMLInputElement).checked)} />
        Start new zines with no image or text frames
      </label>
      <div class="note">Off (the default): new zines get an image frame on the front and back covers, a blank inside front cover, and one text story threaded through the pages after it.</div>
      <div class="actions">
        <button onclick={() => (showPreferences = false)}>Cancel</button>
        <button onclick={savePreferences}>Save</button>
      </div>
    </div>
  </div>
{/if}
