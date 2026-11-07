<script setup lang="ts">
import Counter from "./components/Counter.vue";
import Header from "./components/Header.vue";
import { useCountStore } from "./stores/count";

const store = useCountStore();
</script>

<template>
  <Header title="Hello from Vue" />
  <Counter :start="2" @change="store.record" />
  <p class="total">Last change: {{ store.total }}</p>
</template>
