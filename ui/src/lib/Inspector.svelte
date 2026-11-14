<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type ImageFit } from "./api";
  import { app, run } from "./state.svelte";

  const selection = $derived(app.selection);

  function commitRect(field: "x" | "y" | "w" | "h", event: Event) {
    if (!selection) return;
    const value = Number((event.target as HTMLInputElement).value);
    if (!Number.isFinite(value)) return;
    const rect = { ...selection.rect, [field]: value };
    run(() => api.setFrameRect(selection.page, selection.frame_id, rect));
  }

  async function placeImage() {
    if (!selection) return;
    const chosen = await open({
      title: "Place an image",
      multiple: false,
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "tif", "tiff"] }],
    });
    if (!chosen) return;
    await run(() => api.placeImage(selection.page, selection.frame_id, chosen as string));
  }

  function setFit(fit: ImageFit) {
    if (!selection) return;
    run(() => api.setImageFit(selection.page, selection.frame_id, fit));
  }

  function crop(dx: number, dy: number, factor: number) {
    if (!selection) return;
    run(() => api.adjustCrop(selection.page, selection.frame_id, dx, dy, factor));
  }

  const kindName: Record<string, string> = {
    text: "Text frame",
    image: "Image frame",
    rectangle: "Rectangle",
    rule: "Rule",
    page_number: "Page number",
  };
</script>

{#if selection}
  <h2>{kindName[selection.kind] ?? selection.kind}</h2>
  <div class="path">{selection.frame_id} · page {selection.page + 1}</div>
  <div class="grid" style="margin-top: 10px;">
    <label>X (mm)<input type="number" step="0.5" value={selection.rect.x} onchange={(e) => commitRect("x", e)} /></label>
    <label>Y (mm)<input type="number" step="0.5" value={selection.rect.y} onchange={(e) => commitRect("y", e)} /></label>
    <label>Width (mm)<input type="number" step="0.5" min="2" value={selection.rect.w} onchange={(e) => commitRect("w", e)} /></label>
    <label>Height (mm)<input type="number" step="0.5" min="2" value={selection.rect.h} onchange={(e) => commitRect("h", e)} /></label>
  </div>

  {#if selection.kind === "image"}
    <div class="row">
      <button onclick={placeImage}>Place image…</button>
    </div>
    <div class="path">{selection.image ?? "No image placed"}</div>
    {#if selection.image}
      <div class="row">
        <label style="flex: 1">
          Fit
          <select value={selection.fit} onchange={(e) => setFit((e.target as HTMLSelectElement).value as ImageFit)}>
            <option value="fill">Fill the frame</option>
            <option value="fit">Fit inside the frame</option>
            <option value="manual">Manual crop</option>
          </select>
        </label>
      </div>
      <div class="row">
        <span style="font-size: 11px; color: var(--muted); width: 100%;">Crop: move the picture by 1 mm, or scale it</span>
        <button onclick={() => crop(-1, 0, 1)} title="Move left">←</button>
        <button onclick={() => crop(1, 0, 1)} title="Move right">→</button>
        <button onclick={() => crop(0, -1, 1)} title="Move up">↑</button>
        <button onclick={() => crop(0, 1, 1)} title="Move down">↓</button>
        <button onclick={() => crop(0, 0, 1 / 1.1)} title="Smaller">−</button>
        <button onclick={() => crop(0, 0, 1.1)} title="Larger">+</button>
      </div>
    {/if}
  {:else if selection.kind === "text"}
    <div class="path">Story "{selection.story ?? "—"}", frame {(selection.thread_index ?? 0) + 1} of its thread</div>
  {/if}

  <div class="row" style="font-size: 11px; color: var(--muted);">
    Drag to move, drag a handle to resize. Arrow keys move by 0.5 mm, with Shift by 5 mm. Escape deselects.
  </div>
{:else if app.view}
  <h2>Document</h2>
  <div class="path">{app.view.folder}</div>
  <div class="path">{app.view.pages.length} pages, {app.view.pages[0]?.width} × {app.view.pages[0]?.height} mm</div>
  <div class="row" style="font-size: 11px; color: var(--muted);">Click a frame to select it.</div>
{:else}
  <h2>Gino's</h2>
  <div class="path">No document open.</div>
{/if}
