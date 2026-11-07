<script setup lang="ts">
import logo from "../assets/logo.png";

defineProps<{ title: string }>();
</script>

<template>
  <header class="site-header">
    <img :src="logo" alt="logo" />
    <h1>{{ title }}</h1>
  </header>
</template>
