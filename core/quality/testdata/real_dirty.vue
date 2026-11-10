<template>
  <ul>
    <li v-for="item in items" :key="items.length">entry</li>
  </ul>
</template>

<script setup lang="ts">
const items: string[] = ["a"];
</script>
