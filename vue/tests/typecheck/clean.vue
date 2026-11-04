<script setup lang="ts">
import { double } from "./lib.js";

const four: number = double(2);
</script>

<template>
  <div>{{ four }}</div>
</template>
