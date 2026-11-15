<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type Arrange, type ImageFit, type ImageInfo } from "./api";
  import { app, run } from "./state.svelte";
  import StyleEditor from "./StyleEditor.svelte";
  import DocumentPanel from "./DocumentPanel.svelte";

  const selection = $derived(app.selection);

  // Facts about the placed image. They change with the image, the frame's
  // size and the crop, so they are fetched again (after a short pause, not on
  // every step of a drag) whenever one of those changes.
  let imageInfo = $state<ImageInfo | null>(null);
  const imageKey = $derived(
    selection?.kind === "image" && selection.image
      ? [selection.page, selection.frame_id, selection.image, selection.fit, selection.rect.w, selection.rect.h, app.canUndo].join("|")
      : null,
  );
  $effect(() => {
    const key = imageKey;
    const target = selection;
    if (!key || !target) {
      imageInfo = null;
      return;
    }
    const timer = setTimeout(async () => {
      try {
        imageInfo = await api.imageInfo(target.page, target.frame_id);
      } catch {
        imageInfo = null;
      }
    }, 150);
    return () => clearTimeout(timer);
  });

  /** Under 150 dpi a picture looks soft in print; under 250 it may. */
  function quality(dpi: number): string {
    if (dpi < 150) return "low for print";
    if (dpi < 250) return "fair for print";
    return "good for print";
  }

  function number(event: Event): number | null {
    const value = Number((event.target as HTMLInputElement).value);
    return Number.isFinite(value) ? value : null;
  }

  function commitRect(field: "x" | "y" | "w" | "h", event: Event) {
    if (!selection) return;
    const value = number(event);
    if (value === null) return;
    const rect = { ...selection.rect, [field]: value };
    run(() => api.setFrameRect(selection.page, selection.frame_id, rect));
  }

  function appearance(change: { stroke_width?: number; grey?: number; filled?: boolean }) {
    if (!selection) return;
    const stroke = change.stroke_width ?? selection.stroke_width;
    const grey = change.grey ?? selection.grey;
    const filled = change.filled ?? selection.filled;
    run(() => api.setFrameAppearance(selection.page, selection.frame_id, stroke, grey, filled));
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

  function arrange(how: Arrange) {
    run(() => api.arrange(how));
  }

  function startThreading() {
    app.threading = true;
    app.say("Click a text frame to continue the story into. Escape cancels.");
  }

  const kindName: Record<string, string> = {
    text: "Text frame",
    image: "Image frame",
    rectangle: "Rectangle",
    rule: "Line",
    page_number: "Page number",
  };
</script>

{#if selection}
  {#if app.view}
    <div class="doc-title">{app.view.title}{app.dirty ? " •" : ""}</div>
  {/if}
  <h2>{kindName[selection.kind] ?? selection.kind}</h2>
  <div class="path">{selection.frame_id} · page {selection.page + 1}</div>
  <div class="grid" style="margin-top: 10px;">
    <label>X (mm)<input type="number" step="0.5" value={selection.rect.x} onchange={(e) => commitRect("x", e)} /></label>
    <label>Y (mm)<input type="number" step="0.5" value={selection.rect.y} onchange={(e) => commitRect("y", e)} /></label>
    <label>Width (mm)<input type="number" step="0.5" min="0" value={selection.rect.w} onchange={(e) => commitRect("w", e)} /></label>
    <label>Height (mm)<input type="number" step="0.5" min="0" value={selection.rect.h} onchange={(e) => commitRect("h", e)} /></label>
  </div>

  {#if selection.kind === "image"}
    <div class="row">
      <button onclick={placeImage}>Place image…</button>
    </div>
    <div class="path">{selection.image ?? "No image placed"}</div>
    {#if selection.image && imageInfo}
      <figure class="original">
        <img src={imageInfo.thumbnail} alt="The whole original, uncropped" />
        <figcaption>Original, uncropped</figcaption>
      </figure>
      <dl class="facts">
        <dt>Pixels</dt>
        <dd>{imageInfo.pixel_width} × {imageInfo.pixel_height}</dd>
        <dt>File says</dt>
        <dd>{imageInfo.stated_dpi ? `${Math.round(imageInfo.stated_dpi)} dpi` : "no resolution stated"}</dd>
        <dt>In this frame</dt>
        <dd class:warn={imageInfo.effective_dpi < 150}>
          {Math.round(imageInfo.effective_dpi)} dpi, {quality(imageInfo.effective_dpi)}
        </dd>
      </dl>
      <div class="row" style="margin-top: 6px;">
        <button onclick={() => imageInfo && api.openInDefaultApp(imageInfo.path).catch((error) => app.say(String(error)))}>Open original</button>
      </div>
    {/if}
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
    <div class="path">
      Story "{selection.story ?? "—"}", frame {selection.thread_position + 1} of {selection.thread_length}
    </div>
    <div class="row">
      {#if !app.text}
        <button onclick={() => run(() => api.editFrame(selection.page, selection.frame_id))}>Edit text</button>
      {:else}
        <button onclick={() => run(api.endEditing)}>Done editing</button>
      {/if}
      <button class:active={app.threading} onclick={startThreading} title="Continue this story into another text frame">Thread…</button>
      <button
        disabled={selection.thread_position + 1 >= selection.thread_length}
        onclick={() => run(api.breakThreadAfter)}
        title="The frames after this one get their own story"
      >
        Break thread after
      </button>
    </div>
  {:else if selection.kind === "rectangle" || selection.kind === "rule"}
    <div class="grid" style="margin-top: 10px;">
      <label>Stroke (mm)<input type="number" step="0.05" min="0" value={selection.stroke_width} onchange={(e) => { const v = number(e); if (v !== null) appearance({ stroke_width: v }); }} /></label>
      <label>
        Grey (%)
        <input
          type="number"
          step="5"
          min="0"
          max="100"
          title="0 is black, 100 is white"
          value={Math.round((1 - selection.grey) * 100)}
          onchange={(e) => { const v = number(e); if (v !== null) appearance({ grey: 1 - v / 100 }); }}
        />
      </label>
      {#if selection.kind === "rectangle"}
        <label class="check"><input type="checkbox" checked={selection.filled} onchange={(e) => appearance({ filled: (e.target as HTMLInputElement).checked })} />Filled</label>
      {/if}
    </div>
    <div class="path">Grey is the amount of ink: 100 % is black.</div>
  {/if}

  {#if app.text}
    <StyleEditor />
  {/if}

  <section>
    <h2>Arrange</h2>
    <div class="path" style="margin-top: 0;">Layer {selection.stack_index + 1} of {selection.stack_count} on this page</div>
    <div class="row">
      <button onclick={() => arrange("back")} disabled={selection.stack_index === 0}>To back</button>
      <button onclick={() => arrange("backward")} disabled={selection.stack_index === 0}>Backward</button>
      <button onclick={() => arrange("forward")} disabled={selection.stack_index + 1 >= selection.stack_count}>Forward</button>
      <button onclick={() => arrange("front")} disabled={selection.stack_index + 1 >= selection.stack_count}>To front</button>
    </div>
    <div class="row">
      <button onclick={() => run(api.deleteFrame)}>Delete frame</button>
    </div>
  </section>

  <div class="row" style="font-size: 11px; color: var(--muted);">
    {#if app.text}
      Typing goes into the story. Escape stops editing.
    {:else}
      Drag to move, drag a handle to resize. Arrow keys move by 0.5 mm, with Shift by 5 mm. Enter edits a text frame. Delete removes the frame.
    {/if}
  </div>
{:else}
  <DocumentPanel />
{/if}
