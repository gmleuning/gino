<!-- Minimise, maximise and close, drawn in the bar because the window has no
     Windows title bar. The bar itself is the drag region. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";

  let maximized = $state(false);

  onMount(() => {
    const win = getCurrentWindow();
    const update = async () => (maximized = await win.isMaximized());
    update();
    const off = win.onResized(update);
    return () => {
      off.then((unlisten) => unlisten());
    };
  });

  const win = () => getCurrentWindow();
</script>

<div class="window-controls">
  <button type="button" title="Minimise" aria-label="Minimise" onclick={() => win().minimize()}>
    <svg viewBox="0 0 10 10" width="10" height="10" aria-hidden="true"><path d="M0 5.5h10" stroke="currentColor" /></svg>
  </button>
  <button
    type="button"
    title={maximized ? "Restore" : "Maximise"}
    aria-label={maximized ? "Restore" : "Maximise"}
    onclick={() => win().toggleMaximize()}
  >
    {#if maximized}
      <svg viewBox="0 0 10 10" width="10" height="10" aria-hidden="true"
        ><path d="M0.5 2.5h7v7h-7z M2.5 2.5v-2h7v7h-2" fill="none" stroke="currentColor" /></svg
      >
    {:else}
      <svg viewBox="0 0 10 10" width="10" height="10" aria-hidden="true"><path d="M0.5 0.5h9v9h-9z" fill="none" stroke="currentColor" /></svg>
    {/if}
  </button>
  <button type="button" class="close" title="Close" aria-label="Close" onclick={() => win().close()}>
    <svg viewBox="0 0 10 10" width="10" height="10" aria-hidden="true"><path d="M0.5 0.5l9 9M9.5 0.5l-9 9" stroke="currentColor" /></svg>
  </button>
</div>
