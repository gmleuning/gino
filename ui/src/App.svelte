<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { api } from "./lib/api";
  import { app, run } from "./lib/state.svelte";
  import PageStrip from "./lib/PageStrip.svelte";
  import Canvas from "./lib/Canvas.svelte";
  import Inspector from "./lib/Inspector.svelte";
  import ImeTest from "./lib/ImeTest.svelte";

  let canvas: Canvas | undefined = $state();

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
      app.openView(view);
      app.say(`Opened ${view.title}`);
      setTimeout(() => canvas?.fit(), 0);
    } catch (error) {
      app.say(String(error));
    }
  }

  async function exportReader() {
    try {
      const suggested = await api.defaultExportPath();
      const target = await save({
        title: "Export reader spreads",
        defaultPath: suggested,
        filters: [{ name: "PDF", extensions: ["pdf"] }],
      });
      if (!target) return;
      const result = await api.exportReader(target);
      const warning =
        result.overflows.length > 0
          ? ` Overflow: ${result.overflows.map((o) => `${o.story} (${o.characters} characters)`).join(", ")}.`
          : "";
      app.say(`Exported ${result.path}.${warning}`);
    } catch (error) {
      app.say(String(error));
    }
  }

  function onKeydown(event: KeyboardEvent) {
    const editing = (event.target as HTMLElement)?.matches?.("input, textarea, select, [contenteditable]");
    if (event.ctrlKey && event.key.toLowerCase() === "o") {
      event.preventDefault();
      openDocument();
    } else if (event.ctrlKey && event.key.toLowerCase() === "s" && app.view) {
      event.preventDefault();
      run(api.save).then(() => app.say("Saved"));
    } else if (event.ctrlKey && event.key.toLowerCase() === "z" && app.view && !editing) {
      event.preventDefault();
      run(event.shiftKey ? api.redo : api.undo);
    } else if (event.ctrlKey && event.key.toLowerCase() === "y" && app.view && !editing) {
      event.preventDefault();
      run(api.redo);
    } else if (event.ctrlKey && event.key.toLowerCase() === "e" && app.view) {
      event.preventDefault();
      exportReader();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="layout">
  <div class="topbar">
    <span class="title">Gino's{app.view ? ` — ${app.view.title}${app.dirty ? " •" : ""}` : ""}</span>
    <button onclick={() => openDocument()}>Open…</button>
    <button disabled={!app.view || !app.dirty} onclick={() => run(api.save).then(() => app.say("Saved"))}>Save</button>
    <button disabled={!app.canUndo} onclick={() => run(api.undo)}>Undo</button>
    <button disabled={!app.canRedo} onclick={() => run(api.redo)}>Redo</button>
    <button disabled={!app.view} onclick={exportReader}>Export reader spreads…</button>
    <span class="spacer"></span>
    <button disabled={!app.view} onclick={() => (app.showGuides = !app.showGuides)}>
      {app.showGuides ? "Hide guides" : "Show guides"}
    </button>
    <button disabled={!app.view} onclick={() => canvas?.zoomBy(1 / 1.25)}>−</button>
    <button disabled={!app.view} onclick={() => canvas?.fit()}>Fit</button>
    <button disabled={!app.view} onclick={() => canvas?.zoomBy(1.25)}>+</button>
    <button disabled={!app.view} onclick={() => (app.showImeTest = !app.showImeTest)}>Korean input test</button>
  </div>

  <aside class="strip">
    <PageStrip />
  </aside>

  <main class="canvas-wrap" style="position: relative;">
    {#if app.view}
      <Canvas bind:this={canvas} />
      {#if app.showImeTest}
        <ImeTest />
      {/if}
    {:else}
      <div class="empty">
        <div>
          <p>Open a <code>.zine</code> folder to begin.</p>
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
      <span class="warn">Overflow: story "{overflow.story}" has {overflow.characters} characters that do not fit.</span>
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
