<script lang="ts">
  let { count = "not a number" }: { count?: number } = $props();
</script>

<span>{count}</span>
