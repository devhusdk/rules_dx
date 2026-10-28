<script setup lang="ts">
const count: number = "not a number";
defineProps<{ count: typeof count }>();
</script>

<template>
  <span>{{ count }}</span>
</template>
