<script lang="ts">
  // The canvas shows the current spread as one <svg> whose user unit is a
  // millimetre. It draws what the core sent and forwards pointer events in
  // page millimetres; it never computes a line break or a frame position.

  import { api } from "./api";
  import { app, frameKey, run } from "./state.svelte";
  import { fragment } from "./svg";

  const HANDLE_PX = 7;
  const SNAP_PX = 6;
  const NUDGE_MM = 0.5;
  const NUDGE_LARGE_MM = 5;

  let wrapper: HTMLElement | undefined = $state();
  let svgElement: SVGSVGElement | undefined = $state();
  let dragging = $state(false);
  let moveInFlight = false;
  let pendingMove: [number, number] | null = null;

  const spread = $derived(app.view?.spreads[app.currentSpread] ?? null);
  const pxWidth = $derived(spread ? spread.width * app.zoom : 0);
  const pxHeight = $derived(spread ? spread.height * app.zoom : 0);
  const handleMm = $derived(HANDLE_PX / app.zoom);

  /** The selection's page offset within the current spread, if it is shown. */
  const selectionOffset = $derived.by(() => {
    if (!app.selection || !spread) return null;
    const entry = spread.pages.find(([p]) => p === app.selection!.page);
    return entry ? entry[1] : null;
  });

  export function fit() {
    if (!wrapper || !spread) return;
    const available = Math.max(wrapper.clientWidth - 60, 100);
    const availableHeight = Math.max(wrapper.clientHeight - 60, 100);
    app.zoom = Math.min(available / spread.width, availableHeight / spread.height);
  }

  export function zoomBy(factor: number) {
    app.zoom = Math.min(Math.max(app.zoom * factor, 0.5), 40);
  }

  /** Converts a pointer event to spread millimetres. */
  function toSpread(event: PointerEvent): [number, number] | null {
    if (!svgElement) return null;
    const matrix = svgElement.getScreenCTM();
    if (!matrix) return null;
    const point = new DOMPoint(event.clientX, event.clientY).matrixTransform(matrix.inverse());
    return [point.x, point.y];
  }

  /** Which page of the spread a spread-x falls on, and the page-local coordinates. */
  function toPage(x: number, y: number): { page: number; x: number; y: number } | null {
    if (!spread || !app.view) return null;
    let chosen = spread.pages[0];
    for (const entry of spread.pages) {
      if (x >= entry[1]) chosen = entry;
    }
    return { page: chosen[0], x: x - chosen[1], y };
  }

  async function onPointerDown(event: PointerEvent) {
    if (event.button !== 0) return;
    wrapper?.focus();
    const spreadPoint = toSpread(event);
    if (!spreadPoint) return;
    const local = toPage(...spreadPoint);
    if (!local) return;
    (event.currentTarget as Element).setPointerCapture(event.pointerId);
    dragging = true;
    await run(() => api.pointerDown(local.page, local.x, local.y, SNAP_PX / app.zoom));
  }

  function onPointerMove(event: PointerEvent) {
    if (!dragging) return;
    const spreadPoint = toSpread(event);
    if (!spreadPoint) return;
    const local = toPage(...spreadPoint);
    if (!local) return;
    pendingMove = [local.x, local.y];
    flushMove();
  }

  async function flushMove() {
    if (moveInFlight || !pendingMove) return;
    moveInFlight = true;
    const [x, y] = pendingMove;
    pendingMove = null;
    await run(() => api.pointerMove(x, y));
    moveInFlight = false;
    if (pendingMove) flushMove();
  }

  async function onPointerUp(event: PointerEvent) {
    if (!dragging) return;
    dragging = false;
    pendingMove = null;
    (event.currentTarget as Element).releasePointerCapture(event.pointerId);
    // Wait for any move still being applied before committing.
    while (moveInFlight) await new Promise((resolve) => setTimeout(resolve, 5));
    await run(api.pointerUp);
    app.snap = null;
  }

  function onWheel(event: WheelEvent) {
    if (!event.ctrlKey) return;
    event.preventDefault();
    zoomBy(event.deltaY < 0 ? 1.1 : 1 / 1.1);
  }

  async function onKeyDown(event: KeyboardEvent) {
    if (!app.view) return;
    const step = event.shiftKey ? NUDGE_LARGE_MM : NUDGE_MM;
    switch (event.key) {
      case "ArrowLeft":
        event.preventDefault();
        await run(() => api.nudge(-step, 0));
        break;
      case "ArrowRight":
        event.preventDefault();
        await run(() => api.nudge(step, 0));
        break;
      case "ArrowUp":
        event.preventDefault();
        await run(() => api.nudge(0, -step));
        break;
      case "ArrowDown":
        event.preventDefault();
        await run(() => api.nudge(0, step));
        break;
      case "Escape":
        if (dragging) {
          dragging = false;
          await run(api.cancelDrag);
        } else if (app.selection) {
          await run(() => api.selectFrame(app.selection!.page, null));
        }
        break;
      case "PageDown":
        event.preventDefault();
        app.currentSpread = Math.min(app.currentSpread + 1, app.view.spreads.length - 1);
        break;
      case "PageUp":
        event.preventDefault();
        app.currentSpread = Math.max(app.currentSpread - 1, 0);
        break;
      case "0":
        if (event.ctrlKey) {
          event.preventDefault();
          fit();
        }
        break;
      case "=":
      case "+":
        if (event.ctrlKey) {
          event.preventDefault();
          zoomBy(1.25);
        }
        break;
      case "-":
        if (event.ctrlKey) {
          event.preventDefault();
          zoomBy(1 / 1.25);
        }
        break;
    }
  }

  // Follow the selection to its spread when it is elsewhere.
  $effect(() => {
    if (app.selection) {
      const index = app.spreadOfPage(app.selection.page);
      if (index !== app.currentSpread) app.currentSpread = index;
    }
  });
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex a11y_no_noninteractive_element_interactions -->
<div
  class="canvas-wrap"
  style="height: 100%;"
  tabindex="0"
  role="application"
  aria-label="Canvas"
  bind:this={wrapper}
  onkeydown={onKeyDown}
  onwheel={onWheel}
>
  {#if spread && app.view}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <svg
      class="canvas"
      role="img"
      aria-label="Spread"
      bind:this={svgElement}
      width={pxWidth}
      height={pxHeight}
      viewBox={`0 0 ${spread.width} ${spread.height}`}
      xmlns="http://www.w3.org/2000/svg"
      style:cursor={dragging ? "move" : "default"}
      onpointerdown={onPointerDown}
      onpointermove={onPointerMove}
      onpointerup={onPointerUp}
      onpointercancel={onPointerUp}
    >
      {#each spread.pages as [pageIndex, dx]}
        {@const page = app.view.pages[pageIndex]}
        <g transform={`translate(${dx} 0)`}>
          <rect width={page.width} height={page.height} fill="#fff" stroke="#b9b5ac" vector-effect="non-scaling-stroke" />
          {#each page.frame_ids as frameId (frameId)}
            <g use:fragment={app.frames.get(frameKey(pageIndex, frameId))?.svg ?? ""}></g>
          {/each}
          {#if app.showGuides}
            <g use:fragment={page.guides_svg}></g>
            {#each page.frame_ids as frameId (frameId)}
              {@const patch = app.frames.get(frameKey(pageIndex, frameId))}
              {#if patch}
                <rect
                  x={patch.rect.x}
                  y={patch.rect.y}
                  width={patch.rect.w}
                  height={patch.rect.h}
                  fill="none"
                  stroke={patch.kind === "text" ? "#9bb8e6" : "#cfc9bd"}
                  stroke-dasharray="3 2"
                  stroke-width="1"
                  vector-effect="non-scaling-stroke"
                />
              {/if}
            {/each}
          {/if}
          {#if app.snap && app.snap.page === pageIndex}
            {#if app.snap.x !== null}
              <line x1={app.snap.x} y1="0" x2={app.snap.x} y2={page.height} stroke="#e0338a" stroke-width="1" vector-effect="non-scaling-stroke" />
            {/if}
            {#if app.snap.y !== null}
              <line x1="0" y1={app.snap.y} x2={page.width} y2={app.snap.y} stroke="#e0338a" stroke-width="1" vector-effect="non-scaling-stroke" />
            {/if}
          {/if}
        </g>
      {/each}

      {#if app.selection && selectionOffset !== null}
        {@const r = app.selection.rect}
        <g transform={`translate(${selectionOffset} 0)`} pointer-events="none">
          <rect x={r.x} y={r.y} width={r.w} height={r.h} fill="none" stroke="#2f6fd6" stroke-width="1.5" vector-effect="non-scaling-stroke" />
          {#each [
            [r.x, r.y],
            [r.x + r.w / 2, r.y],
            [r.x + r.w, r.y],
            [r.x + r.w, r.y + r.h / 2],
            [r.x + r.w, r.y + r.h],
            [r.x + r.w / 2, r.y + r.h],
            [r.x, r.y + r.h],
            [r.x, r.y + r.h / 2],
          ] as [hx, hy]}
            <rect
              x={hx - handleMm / 2}
              y={hy - handleMm / 2}
              width={handleMm}
              height={handleMm}
              fill="#fff"
              stroke="#2f6fd6"
              stroke-width="1"
              vector-effect="non-scaling-stroke"
            />
          {/each}
        </g>
      {/if}
    </svg>
  {/if}
</div>
