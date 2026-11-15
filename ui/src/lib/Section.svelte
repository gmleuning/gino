<!-- A panel section whose title opens and closes it. Closed sections stay
     closed while the program runs, whichever frame is selected. -->
<script module lang="ts">
  const closed = $state<Record<string, boolean>>({});
</script>

<script lang="ts">
  import type { Snippet } from "svelte";

  let { title, children }: { title: string; children: Snippet } = $props();
  const open = $derived(!closed[title]);
</script>

<section class="panel-section" class:closed={!open}>
  <h2>
    <button type="button" class="section-toggle" aria-expanded={open} onclick={() => (closed[title] = open)}>
      <svg viewBox="0 0 10 10" width="8" height="8" aria-hidden="true"><path d="M2 3.2 5 6.6 8 3.2" fill="none" stroke="currentColor" stroke-width="1.5" /></svg>
      {title}
    </button>
  </h2>
  {#if open}
    {@render children()}
  {/if}
</section>
