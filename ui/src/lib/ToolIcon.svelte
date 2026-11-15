<!-- Tool pictures, 20 × 20, from docs/brand/theme/tool-*.svg. currentColor lets the bar invert them. -->
<script lang="ts">
  import type { Tool } from "./api";

  let { tool }: { tool: Tool } = $props();
</script>

<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" width="20" height="20" aria-hidden="true">
  {#if tool === "select"}
    <path fill="currentColor" d="M5 2.2v14.6l3.7-3.5 2.3 5.2 2.2-1-2.3-5.1h5z" />
  {:else if tool === "text"}
    <path fill="currentColor" d="M3.5 3.5h13v3.6h-1.4V5.4h-3.9v9.5h1.9v1.6H6.9v-1.6h1.9V5.4H4.9v1.7H3.5z" />
  {:else if tool === "image"}
    <g fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round">
      <rect x="2.8" y="4" width="14.4" height="12" />
      <path d="M2.8 14l4.2-4.2 3.2 3.2 2.3-2.3 4.7 4.7" />
    </g>
    <circle cx="13" cy="7.6" r="1.4" fill="currentColor" />
  {:else if tool === "rule"}
    <path fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="square" d="M4 16L16 4" />
  {/if}
</svg>
