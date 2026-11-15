<script lang="ts">
  // Paragraph and character styles for the text being edited. Applying a
  // style and editing a style's fields are both commands in the core; every
  // paragraph using a style reflows when it changes.

  import { api, type Alignment, type CharacterStyle, type KoreanBreak, type ParagraphStyle } from "./api";
  import { app, run } from "./state.svelte";
  import { prefs } from "./prefs.svelte";

  const text = $derived(app.text);
  const paragraphStyle = $derived(app.paragraphStyles.find((style) => style.id === text?.paragraph_style) ?? null);
  const characterStyle = $derived(app.characterStyles.find((style) => style.id === text?.character_style) ?? null);

  let newParagraphName = $state("");
  let newCharacterName = $state("");

  function families(current: string): string[] {
    return app.fontFamilies.includes(current) ? app.fontFamilies : [current, ...app.fontFamilies];
  }

  function number(event: Event): number | null {
    const value = Number((event.target as HTMLInputElement).value);
    return Number.isFinite(value) ? value : null;
  }

  function setParagraph<K extends keyof ParagraphStyle>(field: K, value: ParagraphStyle[K] | null) {
    if (!paragraphStyle || value === null) return;
    const style = { ...paragraphStyle, [field]: value };
    run(() => api.updateParagraphStyle(style));
  }

  function setCharacter(change: Partial<CharacterStyle>) {
    if (!characterStyle) return;
    const style: CharacterStyle = { ...characterStyle, ...change };
    run(() => api.updateCharacterStyle(style));
  }

  async function createParagraph() {
    const id = newParagraphName.trim();
    if (!id || !paragraphStyle) return;
    const created = await run(() => api.createParagraphStyle(id, paragraphStyle.id));
    if (created) {
      newParagraphName = "";
      await run(() => api.applyParagraphStyle(id));
    }
  }

  async function createCharacter() {
    const id = newCharacterName.trim();
    if (!id) return;
    const created = await run(() => api.createCharacterStyle(id));
    if (created) {
      newCharacterName = "";
      await run(() => api.applyCharacterStyle(id));
    }
  }
</script>

{#if text}
  <section>
    <h2>Paragraph style</h2>
    <select value={text.paragraph_style} onchange={(e) => run(() => api.applyParagraphStyle((e.target as HTMLSelectElement).value))}>
      {#each app.paragraphStyles as style (style.id)}
        <option value={style.id}>{style.id}</option>
      {/each}
    </select>

    {#if paragraphStyle}
      <div class="grid" style="margin-top: 8px;">
        <label style="grid-column: 1 / -1;">
          Latin font
          <select value={paragraphStyle.latin_font} onchange={(e) => setParagraph("latin_font", (e.target as HTMLSelectElement).value)}>
            {#each families(paragraphStyle.latin_font) as family}
              <option value={family}>{family}</option>
            {/each}
          </select>
        </label>
        {#if prefs.current.show_korean}
        <label style="grid-column: 1 / -1;">
          Korean font
          <select value={paragraphStyle.korean_font} onchange={(e) => setParagraph("korean_font", (e.target as HTMLSelectElement).value)}>
            {#each families(paragraphStyle.korean_font) as family}
              <option value={family}>{family}</option>
            {/each}
          </select>
        </label>
        {/if}
        <label>Size (pt)<input type="number" step="0.5" min="2" value={paragraphStyle.size} onchange={(e) => setParagraph("size", number(e))} /></label>
        <label>Leading (pt)<input type="number" step="0.5" min="2" value={paragraphStyle.leading} onchange={(e) => setParagraph("leading", number(e))} /></label>
        {#if prefs.current.show_korean}
        <label>
          Korean scale (%)
          <input
            type="number"
            step="1"
            min="50"
            max="200"
            value={Math.round(paragraphStyle.korean_scale * 100)}
            onchange={(e) => {
              const value = number(e);
              setParagraph("korean_scale", value === null ? null : value / 100);
            }}
          />
        </label>
        {/if}
        <label>Tracking (pt)<input type="number" step="0.1" value={paragraphStyle.tracking} onchange={(e) => setParagraph("tracking", number(e))} /></label>
        <label>
          Alignment
          <select value={paragraphStyle.alignment} onchange={(e) => setParagraph("alignment", (e.target as HTMLSelectElement).value as Alignment)}>
            <option value="left">Left</option>
            <option value="center">Centre</option>
            <option value="right">Right</option>
            <option value="justify">Justify</option>
          </select>
        </label>
        {#if prefs.current.show_korean}
        <label>
          Korean breaks
          <select value={paragraphStyle.korean_break} onchange={(e) => setParagraph("korean_break", (e.target as HTMLSelectElement).value as KoreanBreak)}>
            <option value="word">Between words</option>
            <option value="syllable">Between syllables</option>
          </select>
        </label>
        {/if}
        <label>Space before (mm)<input type="number" step="0.5" min="0" value={paragraphStyle.space_before} onchange={(e) => setParagraph("space_before", number(e))} /></label>
        <label>Space after (mm)<input type="number" step="0.5" min="0" value={paragraphStyle.space_after} onchange={(e) => setParagraph("space_after", number(e))} /></label>
        <label>First-line indent (mm)<input type="number" step="0.5" value={paragraphStyle.first_line_indent} onchange={(e) => setParagraph("first_line_indent", number(e))} /></label>
      </div>
    {/if}
    <div class="row">
      <input type="text" placeholder="New style name" bind:value={newParagraphName} style="flex: 1;" onkeydown={(e) => e.key === "Enter" && createParagraph()} />
      <button disabled={!newParagraphName.trim()} onclick={createParagraph}>New</button>
    </div>
  </section>

  <section>
    <h2>Character style</h2>
    <select
      value={text.character_style ?? ""}
      onchange={(e) => {
        const value = (e.target as HTMLSelectElement).value;
        run(() => api.applyCharacterStyle(value === "" ? null : value));
      }}
    >
      <option value="">None</option>
      {#each app.characterStyles as style (style.id)}
        <option value={style.id}>{style.id}</option>
      {/each}
    </select>
    <div class="path">Applies to the selected text; with nothing selected, to what you type next.</div>

    {#if characterStyle}
      <div class="grid" style="margin-top: 8px;">
        <label class="check">
          <input type="checkbox" checked={characterStyle.font_style === "italic"} onchange={(e) => setCharacter({ font_style: (e.target as HTMLInputElement).checked ? "italic" : undefined })} />
          Italic
        </label>
        <label class="check">
          <input type="checkbox" checked={characterStyle.underline ?? false} onchange={(e) => setCharacter({ underline: (e.target as HTMLInputElement).checked })} />
          Underline
        </label>
        <label style="grid-column: 1 / -1;">
          Weight
          <select
            value={String(characterStyle.font_weight ?? "")}
            onchange={(e) => {
              const value = (e.target as HTMLSelectElement).value;
              setCharacter({ font_weight: value === "" ? undefined : Number(value) });
            }}
          >
            <option value="">From the paragraph</option>
            <option value="300">Light (300)</option>
            <option value="400">Regular (400)</option>
            <option value="500">Medium (500)</option>
            <option value="700">Bold (700)</option>
            <option value="900">Black (900)</option>
          </select>
        </label>
      </div>
    {/if}
    <div class="row">
      <input type="text" placeholder="New style name" bind:value={newCharacterName} style="flex: 1;" onkeydown={(e) => e.key === "Enter" && createCharacter()} />
      <button disabled={!newCharacterName.trim()} onclick={createCharacter}>New</button>
    </div>
  </section>
{/if}
