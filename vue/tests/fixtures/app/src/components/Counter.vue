<script setup lang="ts">
import { ref } from "vue";

const props = defineProps<{ start: number }>();
const emit = defineEmits<{ change: [number] }>();
const count = ref(props.start);

function increment(): void {
  count.value += 1;
  emit("change", count.value);
}
</script>

<template>
  <button class="counter" type="button" @click="increment">
    Count: {{ count }}
  </button>
</template>

<style scoped>
.counter {
  font-weight: bold;
}
</style>
