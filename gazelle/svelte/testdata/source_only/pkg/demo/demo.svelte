<script>
  import fs from "fs"
  export let title = ""
  const base = import.meta.env.BASE_URL
  import "./helper.svelte"

  let greeting = fs.sep + base + title
</script>

<div class="demo">{greeting}</div>

<style>
  .demo {
    color: black;
  }
</style>
