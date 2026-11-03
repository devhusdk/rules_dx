<script lang="ts">
  import type { Snippet } from "svelte";

  /** The props of a card. */
  interface Props {
    /** Heading of the card. */
    title: string;
    /** Number of visible rows. */
    rows?: number;
    /** Called when the card is selected. */
    onselect?: (id: number) => void;
    /** Main content. */
    children: Snippet;
    /** Content below the main content. */
    footer?: Snippet<[string]>;
  }

  let { title, rows = 3, onselect, children, footer }: Props = $props();
</script>

<section>
  <h2>{title}</h2>
  <button onclick={() => onselect?.(rows)}>{rows}</button>
  {@render children()}
  {@render footer?.(title)}
</section>
