<script lang="ts">
  // The canvas shows the current spread as one <svg> whose user unit is a
  // millimetre. It draws what the core sent and forwards pointer and key
  // events in page millimetres; it never computes a line break or a frame
  // position. Text input goes through a hidden <textarea> placed at the
  // caret, so the Korean input method composes there and its candidate
  // window opens next to the caret.

  import { tick } from "svelte";
  import { api, type Motion, type Tool, type Update } from "./api";
  import { app, frameKey, run } from "./state.svelte";
  import { fragment } from "./svg";

  const HANDLE_PX = 7;
  const SNAP_PX = 6;
  const NUDGE_MM = 0.5;
  const NUDGE_LARGE_MM = 5;
  const MARKER_PX = 12;

  let wrapper: HTMLElement | undefined = $state();
  let svgElement: SVGSVGElement | undefined = $state();
  let textarea: HTMLTextAreaElement | undefined = $state();
  let dragging = $state(false);
  let moveInFlight = false;
  let pendingMove: [number, number] | null = null;
  let composing = false;

  const spread = $derived(app.view?.spreads[app.currentSpread] ?? null);
  const pxWidth = $derived(spread ? spread.width * app.zoom : 0);
  const pxHeight = $derived(spread ? spread.height * app.zoom : 0);
  const handleMm = $derived(HANDLE_PX / app.zoom);
  const markerMm = $derived(MARKER_PX / app.zoom);

  function offsetOf(page: number): number | null {
    const entry = spread?.pages.find(([p]) => p === page);
    return entry ? entry[1] : null;
  }

  const selectionOffset = $derived(app.selection ? offsetOf(app.selection.page) : null);
  const caret = $derived(app.text?.caret ?? null);
  const caretOffset = $derived(caret ? offsetOf(caret.page) : null);

  export function fit() {
    if (!wrapper || !spread) return;
    const available = Math.max(wrapper.clientWidth - 60, 100);
    const availableHeight = Math.max(wrapper.clientHeight - 60, 100);
    app.zoom = Math.min(available / spread.width, availableHeight / spread.height);
  }

  export function zoomBy(factor: number) {
    app.zoom = Math.min(Math.max(app.zoom * factor, 0.5), 40);
  }

  /** Keyboard focus goes to the textarea while a story is being edited. */
  export async function focus() {
    await tick();
    if (app.text) textarea?.focus({ preventScroll: true });
    else wrapper?.focus({ preventScroll: true });
  }

  // Core calls from typing must reach the core in the order they were typed.
  let queue: Promise<unknown> = Promise.resolve();
  function enqueue(action: () => Promise<Update>) {
    queue = queue.then(() => run(action));
    return queue;
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
    // Keep focus where it is (the textarea, while editing) until the core answers.
    event.preventDefault();
    const spreadPoint = toSpread(event);
    if (!spreadPoint) return;
    const local = toPage(...spreadPoint);
    if (!local) return;
    if (app.threading) {
      app.threading = false;
      const update = await enqueue(() => api.threadTo(local.page, local.x, local.y));
      if (update) app.say("Threaded.");
      focus();
      return;
    }
    (event.currentTarget as Element).setPointerCapture(event.pointerId);
    dragging = true;
    const tool: Tool = app.tool;
    await enqueue(() =>
      api.pointerDown(local.page, local.x, local.y, SNAP_PX / app.zoom, tool, event.shiftKey, Math.min(event.detail, 3)),
    );
    focus();
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
    await enqueue(() => api.pointerMove(x, y));
    moveInFlight = false;
    if (pendingMove) flushMove();
  }

  async function onPointerUp(event: PointerEvent) {
    if (!dragging) return;
    dragging = false;
    pendingMove = null;
    (event.currentTarget as Element).releasePointerCapture(event.pointerId);
    while (moveInFlight) await new Promise((resolve) => setTimeout(resolve, 5));
    await enqueue(api.pointerUp);
    app.snap = null;
    // Drawing tools make one frame, then return to selecting.
    app.tool = "select";
    focus();
  }

  function onWheel(event: WheelEvent) {
    if (!event.ctrlKey) return;
    event.preventDefault();
    zoomBy(event.deltaY < 0 ? 1.1 : 1 / 1.1);
  }

  const toolKeys: Record<string, Tool> = { v: "select", t: "text", i: "image", r: "rectangle", l: "rule" };

  /** Keys while frames, not text, have the focus. */
  async function onKeyDown(event: KeyboardEvent) {
    if (!app.view || event.target !== wrapper) return;
    const step = event.shiftKey ? NUDGE_LARGE_MM : NUDGE_MM;
    const key = event.key;
    if (!event.ctrlKey && !event.altKey && toolKeys[key.toLowerCase()]) {
      app.tool = toolKeys[key.toLowerCase()];
      return;
    }
    switch (key) {
      case "ArrowLeft":
        event.preventDefault();
        await enqueue(() => api.nudge(-step, 0));
        break;
      case "ArrowRight":
        event.preventDefault();
        await enqueue(() => api.nudge(step, 0));
        break;
      case "ArrowUp":
        event.preventDefault();
        await enqueue(() => api.nudge(0, -step));
        break;
      case "ArrowDown":
        event.preventDefault();
        await enqueue(() => api.nudge(0, step));
        break;
      case "Delete":
      case "Backspace":
        if (app.selection) {
          event.preventDefault();
          await enqueue(api.deleteFrame);
        }
        break;
      case "Enter":
        if (app.selection?.kind === "text") {
          event.preventDefault();
          const { page, frame_id } = app.selection;
          await enqueue(() => api.editFrame(page, frame_id));
          focus();
        }
        break;
      case "Escape":
        if (app.threading) {
          app.threading = false;
          app.say(null);
        } else if (dragging) {
          dragging = false;
          await enqueue(api.cancelDrag);
        } else if (app.tool !== "select") {
          app.tool = "select";
        } else if (app.selection) {
          await enqueue(() => api.selectFrame(app.selection!.page, null));
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

  // Text input ------------------------------------------------------------

  function onCompositionStart() {
    composing = true;
  }

  function onCompositionUpdate(event: CompositionEvent) {
    const text = event.data ?? "";
    enqueue(() => api.setPreedit(text));
  }

  function onCompositionEnd(event: CompositionEvent) {
    composing = false;
    const text = event.data ?? "";
    enqueue(() => api.commitText(text));
    if (textarea) textarea.value = "";
  }

  function onInput(event: Event) {
    if ((event as InputEvent).isComposing || composing || !textarea) return;
    const text = textarea.value;
    textarea.value = "";
    if (text) enqueue(() => api.insertText(text.replace(/\r\n?/g, "\n")));
  }

  function caretMotion(event: KeyboardEvent): Motion | null {
    const ctrl = event.ctrlKey;
    switch (event.key) {
      case "ArrowLeft":
        return ctrl ? "word_left" : "left";
      case "ArrowRight":
        return ctrl ? "word_right" : "right";
      case "ArrowUp":
        return "up";
      case "ArrowDown":
        return "down";
      case "Home":
        return ctrl ? "story_start" : "line_start";
      case "End":
        return ctrl ? "story_end" : "line_end";
    }
    return null;
  }

  async function onTextKeyDown(event: KeyboardEvent) {
    // The input method owns keys while it composes.
    if (event.isComposing || composing || event.keyCode === 229) return;
    const motion = caretMotion(event);
    if (motion) {
      event.preventDefault();
      enqueue(() => api.moveCaret(motion, event.shiftKey));
      return;
    }
    const key = event.key.toLowerCase();
    if (event.ctrlKey && !event.altKey) {
      if (key === "a") {
        event.preventDefault();
        enqueue(api.selectAll);
      } else if (key === "z") {
        event.preventDefault();
        enqueue(event.shiftKey ? api.redo : api.undo);
      } else if (key === "y") {
        event.preventDefault();
        enqueue(api.redo);
      }
      // Ctrl+C, X, V reach the copy, cut and paste events; Ctrl+S, O, E reach the window.
      if (key !== "backspace" && key !== "delete") return;
    }
    switch (event.key) {
      case "Backspace":
        event.preventDefault();
        enqueue(() => api.deleteText(true, event.ctrlKey));
        break;
      case "Delete":
        event.preventDefault();
        enqueue(() => api.deleteText(false, event.ctrlKey));
        break;
      case "Enter":
        event.preventDefault();
        enqueue(() => api.insertText("\n"));
        break;
      case "Tab":
        event.preventDefault();
        break;
      case "Escape":
        event.preventDefault();
        await enqueue(api.endEditing);
        focus();
        break;
      case "PageDown":
      case "PageUp":
        event.preventDefault();
        break;
    }
  }

  function onCopy(event: ClipboardEvent) {
    const text = app.text?.selected_text ?? "";
    event.preventDefault();
    if (text) event.clipboardData?.setData("text/plain", text);
  }

  function onCut(event: ClipboardEvent) {
    const text = app.text?.selected_text ?? "";
    event.preventDefault();
    if (!text) return;
    event.clipboardData?.setData("text/plain", text);
    enqueue(() => api.deleteText(true, false));
  }

  function onPaste(event: ClipboardEvent) {
    event.preventDefault();
    const text = event.clipboardData?.getData("text/plain") ?? "";
    if (text) enqueue(() => api.insertText(text.replace(/\r\n?/g, "\n")));
  }

  async function startThreadFrom(page: number, frameId: string, event: PointerEvent) {
    event.stopPropagation();
    event.preventDefault();
    await enqueue(() => api.selectFrame(page, frameId));
    app.threading = true;
    app.say("Click a text frame to continue the story into. Escape cancels.");
    wrapper?.focus({ preventScroll: true });
  }

  // Follow the selection, or the caret, to its spread when it is elsewhere.
  $effect(() => {
    const page = app.text?.caret?.page ?? app.selection?.page;
    if (page !== undefined) {
      const index = app.spreadOfPage(page);
      if (index !== app.currentSpread) app.currentSpread = index;
    }
  });

  // Overflow markers on frames shown in this spread.
  const markers = $derived.by(() => {
    if (!spread) return [];
    const result: { page: number; frame: string; x: number; y: number; characters: number }[] = [];
    for (const overflow of app.overflows) {
      const offset = offsetOf(overflow.page);
      const patch = app.frames.get(frameKey(overflow.page, overflow.frame));
      if (offset === null || !patch) continue;
      result.push({
        page: overflow.page,
        frame: overflow.frame,
        x: offset + patch.rect.x + patch.rect.w,
        y: patch.rect.y + patch.rect.h,
        characters: overflow.characters,
      });
    }
    return result;
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
    <div class="sheet" style:width={`${pxWidth}px`} style:height={`${pxHeight}px`}>
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
        style:cursor={app.threading ? "alias" : app.tool !== "select" ? "crosshair" : dragging ? "move" : app.text ? "text" : "default"}
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
                {#if patch && patch.kind !== "rule"}
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
            {#if app.text}
              {#each app.text.selection.filter((box) => box.page === pageIndex) as box}
                <rect x={box.x} y={box.y} width={box.w} height={box.h} fill="#2f6fd6" fill-opacity="0.25" pointer-events="none" />
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

        {#if app.selection && selectionOffset !== null && !app.text}
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

        {#if app.selection && selectionOffset !== null && app.text}
          {@const r = app.selection.rect}
          <rect
            x={selectionOffset + r.x}
            y={r.y}
            width={r.w}
            height={r.h}
            fill="none"
            stroke="#2f6fd6"
            stroke-width="1"
            vector-effect="non-scaling-stroke"
            pointer-events="none"
          />
        {/if}

        {#if caret && caretOffset !== null && !app.text?.selection.length}
          <line
            class="caret"
            x1={caretOffset + caret.x}
            y1={caret.top}
            x2={caretOffset + caret.x}
            y2={caret.bottom}
            stroke="#1e1e1e"
            stroke-width="1.5"
            vector-effect="non-scaling-stroke"
            pointer-events="none"
          />
        {/if}

        {#each markers as marker (marker.page + marker.frame)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <g
            class="overflow-marker"
            role="button"
            tabindex="-1"
            onpointerdown={(event) => startThreadFrom(marker.page, marker.frame, event)}
          >
            <title>{marker.characters} characters do not fit. Click, then click another text frame to continue the story.</title>
            <rect
              x={marker.x - markerMm}
              y={marker.y - markerMm}
              width={markerMm}
              height={markerMm}
              fill="#fff"
              stroke="#b3261e"
              stroke-width="1.5"
              vector-effect="non-scaling-stroke"
            />
            <path
              d={`M ${marker.x - markerMm / 2} ${marker.y - markerMm * 0.8} V ${marker.y - markerMm * 0.2} M ${marker.x - markerMm * 0.8} ${marker.y - markerMm / 2} H ${marker.x - markerMm * 0.2}`}
              stroke="#b3261e"
              stroke-width="1.5"
              vector-effect="non-scaling-stroke"
            />
          </g>
        {/each}
      </svg>

      {#if app.text}
        <!-- The input method composes here; the core draws the result. -->
        <textarea
          class="ime"
          bind:this={textarea}
          autocomplete="off"
          spellcheck="false"
          aria-label="Text input"
          style:left={`${((caretOffset ?? 0) + (caret?.x ?? 0)) * app.zoom}px`}
          style:top={`${(caret?.top ?? 0) * app.zoom}px`}
          style:height={`${Math.max(((caret?.bottom ?? 0) - (caret?.top ?? 0)) * app.zoom, 8)}px`}
          style:font-size={`${Math.max(((caret?.bottom ?? 0) - (caret?.top ?? 0)) * app.zoom * 0.8, 8)}px`}
          oncompositionstart={onCompositionStart}
          oncompositionupdate={onCompositionUpdate}
          oncompositionend={onCompositionEnd}
          oninput={onInput}
          onkeydown={onTextKeyDown}
          oncopy={onCopy}
          oncut={onCut}
          onpaste={onPaste}
          onblur={() => (composing = false)}
        ></textarea>
      {/if}
    </div>
  {/if}
</div>
