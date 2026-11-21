<script setup lang="ts">
import { double, missing } from "./lib.js";

const bad: number = double("x");
</script>

<template>
  <div>{{ bad }}</div>
</template>
