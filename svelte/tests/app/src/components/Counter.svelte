<script lang="ts">
  import clsx from "clsx";

  let {
    start,
    onchange,
  }: { start: number; onchange: (value: number) => void } = $props();
  let count = $state(start);
  let label = $derived(`Count: ${count}`);

  function increment(): void {
    count += 1;
    onchange(count);
  }
</script>

<button
  class={clsx("counter", { big: count > 2 })}
  type="button"
  onclick={increment}
>
  {label}
</button>

<style>
  .counter {
    font-weight: bold;
  }
</style>
