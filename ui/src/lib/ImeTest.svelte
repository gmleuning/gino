<script lang="ts">
  // Milestone 2 throwaway: Korean input-method composition against a frame
  // the core lays out. Two approaches, switchable: a hidden <textarea> that
  // forwards composition events, and the EditContext interface.

  import { onMount } from "svelte";
  import { api, type ImeTestView } from "./api";
  import { app } from "./state.svelte";
  import { fragment } from "./svg";

  const WIDTH_MM = 100;
  const PX_PER_MM = 3.6;
  const HEIGHT_MM = 40;

  type Mode = "textarea" | "editcontext";
  const editContextSupported = typeof (window as any).EditContext === "function";

  let mode = $state<Mode>("textarea");
  let committed = $state("");
  let composing = $state("");
  let view = $state<ImeTestView | null>(null);
  let log = $state<string[]>([]);
  let textarea: HTMLTextAreaElement | undefined = $state();
  let host: HTMLDivElement | undefined = $state();
  let editContext: any = null;
  let compositionStart = 0;

  function note(line: string) {
    log = [...log.slice(-40), line];
  }

  async function render() {
    try {
      view = await api.imeTest(committed, composing, WIDTH_MM);
    } catch (error) {
      app.say(String(error));
    }
  }

  // Textarea approach ------------------------------------------------------

  function onCompositionStart(event: CompositionEvent) {
    note(`compositionstart "${event.data}"`);
    composing = "";
  }

  function onCompositionUpdate(event: CompositionEvent) {
    note(`compositionupdate "${event.data}"`);
    composing = event.data ?? "";
    render();
  }

  function onCompositionEnd(event: CompositionEvent) {
    note(`compositionend "${event.data}"`);
    committed += event.data ?? "";
    composing = "";
    if (textarea) textarea.value = "";
    render();
  }

  function onInput(event: Event) {
    const input = event as InputEvent;
    if (input.isComposing) return;
    if (!textarea) return;
    if (input.inputType === "deleteContentBackward") {
      committed = Array.from(committed).slice(0, -1).join("");
      note("backspace");
    } else if (textarea.value) {
      note(`input "${textarea.value}"`);
      committed += textarea.value;
    }
    textarea.value = "";
    render();
  }

  function onKeyDown(event: KeyboardEvent) {
    if (event.key === "Backspace" && !event.isComposing && textarea && textarea.value === "") {
      event.preventDefault();
      committed = Array.from(committed).slice(0, -1).join("");
      note("backspace");
      render();
    }
  }

  // EditContext approach ---------------------------------------------------

  function attachEditContext() {
    if (!editContextSupported || !host) return;
    const EditContextClass = (window as any).EditContext;
    editContext = new EditContextClass({ text: committed, selectionStart: committed.length, selectionEnd: committed.length });
    (host as any).editContext = editContext;
    editContext.addEventListener("textupdate", (event: any) => {
      const text: string = editContext.text;
      note(`textupdate [${event.updateRangeStart}, ${event.updateRangeEnd}) "${event.text}"`);
      if (editContext.isComposing ?? composingFlag) {
        committed = text.slice(0, compositionStart);
        composing = text.slice(compositionStart);
      } else {
        committed = text;
        composing = "";
      }
      render().then(updateBounds);
    });
    let composingFlag = false;
    editContext.addEventListener("compositionstart", () => {
      composingFlag = true;
      compositionStart = editContext.selectionStart ?? editContext.text.length;
      note(`compositionstart at ${compositionStart}`);
    });
    editContext.addEventListener("compositionend", () => {
      composingFlag = false;
      committed = editContext.text;
      composing = "";
      note("compositionend");
      render().then(updateBounds);
    });
    updateBounds();
  }

  function updateBounds() {
    if (!editContext || !host || !view) return;
    const hostRect = host.getBoundingClientRect();
    editContext.updateControlBounds(hostRect);
    const caret = new DOMRect(
      hostRect.left + view.caret_x * PX_PER_MM,
      hostRect.top + view.caret_top * PX_PER_MM,
      1,
      (view.caret_bottom - view.caret_top) * PX_PER_MM,
    );
    editContext.updateSelectionBounds(caret);
  }

  function focusInput() {
    if (mode === "textarea") textarea?.focus();
    else host?.focus();
  }

  function clearAll() {
    committed = "";
    composing = "";
    if (editContext) editContext.updateText(0, editContext.text.length, "");
    render();
  }

  $effect(() => {
    // Re-attach when the mode changes.
    mode;
    if (mode === "editcontext") attachEditContext();
    else if (host) (host as any).editContext = null;
    setTimeout(focusInput, 0);
  });

  onMount(() => {
    render();
  });
</script>

<div class="ime-panel">
  <h3>Korean input test (milestone 2)</h3>
  <div class="modes">
    <label><input type="radio" bind:group={mode} value="textarea" /> Hidden textarea</label>
    <label><input type="radio" bind:group={mode} value="editcontext" disabled={!editContextSupported} /> EditContext {editContextSupported ? "" : "(not supported by this WebView2)"}</label>
    <button onclick={clearAll} style="margin-left: auto;">Clear</button>
  </div>
  <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
  <div class="frame" style={`width: ${WIDTH_MM * PX_PER_MM}px; height: ${HEIGHT_MM * PX_PER_MM}px;`} onclick={focusInput}>
    <svg width={WIDTH_MM * PX_PER_MM} height={HEIGHT_MM * PX_PER_MM} viewBox={`0 0 ${WIDTH_MM} ${HEIGHT_MM}`} xmlns="http://www.w3.org/2000/svg">
      <g transform="translate(0 2)" use:fragment={view?.svg ?? ""}></g>
      {#if view}
        <line
          x1={view.caret_x}
          y1={view.caret_top + 2}
          x2={view.caret_x}
          y2={view.caret_bottom + 2}
          stroke="#2f6fd6"
          stroke-width="1"
          vector-effect="non-scaling-stroke"
        />
      {/if}
    </svg>
    {#if mode === "textarea"}
      <textarea
        bind:this={textarea}
        style={`left: ${(view?.caret_x ?? 0) * PX_PER_MM}px; top: ${((view?.caret_top ?? 0) + 2) * PX_PER_MM}px;`}
        oncompositionstart={onCompositionStart}
        oncompositionupdate={onCompositionUpdate}
        oncompositionend={onCompositionEnd}
        oninput={onInput}
        onkeydown={onKeyDown}
        autocomplete="off"
        spellcheck="false"
      ></textarea>
    {:else}
      <div class="edit-host" bind:this={host} tabindex="0" role="textbox" aria-label="Korean input test"></div>
    {/if}
  </div>
  <div class="path" style="font-size: 11px; color: var(--muted); margin-top: 6px;">
    Click the frame, switch Windows to Korean (한/영) and type. Composing syllables are underlined; the candidate window should appear at the caret.
  </div>
  <div class="log">{log.join("\n")}</div>
</div>
