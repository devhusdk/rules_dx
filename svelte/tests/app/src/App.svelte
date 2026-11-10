<script lang="ts">
  import Counter from "./components/Counter.svelte";
  import Header from "./components/Header.svelte";

  let total = $state(0);
</script>

<Header title="Hello from Svelte" />
<Counter start={2} onchange={(value: number) => (total = value)} />
<p class="total">Last change: {total}</p>
