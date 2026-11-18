<template>
  <ul>
    <li v-for="(item, index) in items" :key="item">entry</li>
  </ul>
</template>

<script setup>
const items = ["a"];
</script>
