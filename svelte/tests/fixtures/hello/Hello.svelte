<script>
  import { helper } from "./helper.js";

  let name = "world";
  let message = $derived(helper(name));
</script>

<div class="hello">{message}</div>

<style>
  .hello {
    color: black;
  }
</style>
