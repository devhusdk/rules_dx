<script lang="ts">
  import { double, missing } from "./lib.js";

  const bad: number = double("x");
</script>

<div>{bad}</div>
