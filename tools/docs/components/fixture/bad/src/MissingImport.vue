<script setup lang="ts">
import type { Gone } from "./Gone";
defineProps<{ gone: Gone }>();
</script>

<template>
  <span>{{ gone }}</span>
</template>
