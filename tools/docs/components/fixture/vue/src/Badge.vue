<script lang="ts">
import { defineComponent } from "vue";

/** A small status badge. */
export default defineComponent({
  props: {
    /** The badge text. */
    text: { type: String, required: true },
  },
});
</script>

<template>
  <span>{{ text }}</span>
</template>
