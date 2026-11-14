// Svelte action: sets an SVG fragment from the core as the children of an
// SVG group. The fragment is produced by the core, never edited here.

export function fragment(node: SVGGElement, svg: string) {
  node.innerHTML = svg;
  return {
    update(next: string) {
      if (next !== svg) {
        svg = next;
        node.innerHTML = next;
      }
    },
  };
}
