<script lang="ts">
  import { app, frameKey } from "./state.svelte";
  import { fragment } from "./svg";
</script>

{#if app.view}
  {#each app.view.spreads as spread, index}
    <button
      class="thumb"
      class:current={index === app.currentSpread}
      onclick={() => (app.currentSpread = index)}
      title={`Spread ${index + 1}`}
    >
      <svg viewBox={`0 0 ${spread.width} ${spread.height}`} xmlns="http://www.w3.org/2000/svg">
        {#each spread.pages as [pageIndex, dx]}
          {@const page = app.view.pages[pageIndex]}
          <g transform={`translate(${dx} 0)`}>
            <rect width={page.width} height={page.height} fill="#fff" stroke="#c8c4bb" vector-effect="non-scaling-stroke" />
            {#each page.frame_ids as frameId (frameId)}
              <g use:fragment={app.frames.get(frameKey(pageIndex, frameId))?.svg ?? ""}></g>
            {/each}
          </g>
        {/each}
      </svg>
      <div class="label">
        {spread.pages.map(([p]) => p + 1).join("–")}
      </div>
    </button>
  {/each}
{/if}
