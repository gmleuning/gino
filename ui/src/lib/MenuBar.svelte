<script lang="ts" module>
  export interface MenuItem {
    label?: string;
    shortcut?: string;
    action?: () => void;
    disabled?: boolean;
    /** A tick in front of the label, for things shown or hidden. */
    checked?: boolean;
    separator?: boolean;
    submenu?: MenuItem[];
  }

  export interface Menu {
    label: string;
    items: MenuItem[];
  }
</script>

<script lang="ts">
  // A menu bar of dropdowns. A click opens a menu; while one is open,
  // hovering another opens that one instead, as in a desktop menu bar.

  let { menus }: { menus: Menu[] } = $props();

  let openIndex = $state<number | null>(null);
  let openSubmenu = $state<string | null>(null);
  let bar: HTMLElement | undefined = $state();

  function close() {
    openIndex = null;
    openSubmenu = null;
  }

  function choose(item: MenuItem) {
    if (item.disabled || item.submenu || !item.action) return;
    close();
    item.action();
  }

  function onWindowPointerDown(event: PointerEvent) {
    if (openIndex !== null && bar && !bar.contains(event.target as Node)) close();
  }

  function onWindowKeyDown(event: KeyboardEvent) {
    if (openIndex !== null && event.key === "Escape") {
      event.stopPropagation();
      close();
    }
  }
</script>

<svelte:window onpointerdown={onWindowPointerDown} onkeydowncapture={onWindowKeyDown} />

<nav class="menubar" bind:this={bar} aria-label="Menu">
  {#each menus as menu, index}
    <div class="menu">
      <button
        class="menu-title"
        class:open={openIndex === index}
        aria-haspopup="menu"
        aria-expanded={openIndex === index}
        onclick={() => {
          openSubmenu = null;
          openIndex = openIndex === index ? null : index;
        }}
        onpointerenter={() => {
          if (openIndex !== null && openIndex !== index) {
            openSubmenu = null;
            openIndex = index;
          }
        }}>{menu.label}</button
      >
      {#if openIndex === index}
        <div class="dropdown" role="menu">
          {@render list(menu.items, `${index}`)}
        </div>
      {/if}
    </div>
  {/each}
</nav>

{#snippet list(items: MenuItem[], path: string)}
  {#each items as item, position}
    {#if item.separator}
      <div class="separator" role="separator"></div>
    {:else}
      {@const key = `${path}.${position}`}
      <div
        class="item-wrap"
        role="none"
        onpointerenter={() => (openSubmenu = item.submenu ? key : openSubmenu?.startsWith(path + ".") ? null : openSubmenu)}
      >
        <button
          class="item"
          role="menuitem"
          disabled={item.disabled}
          onclick={() => (item.submenu ? (openSubmenu = key) : choose(item))}
        >
          <span class="tick">{item.checked ? "✓" : ""}</span>
          <span class="label">{item.label}</span>
          <span class="shortcut">{item.submenu ? "›" : (item.shortcut ?? "")}</span>
        </button>
        {#if item.submenu && openSubmenu === key}
          <div class="dropdown submenu" role="menu">
            {@render list(item.submenu, key)}
          </div>
        {/if}
      </div>
    {/if}
  {/each}
{/snippet}
