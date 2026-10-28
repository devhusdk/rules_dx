<script setup lang="ts">
defineProps<{ /** Some text. */ text: string }>();
</script>

<template>
  <span>{{ text }}</span>
</template>
