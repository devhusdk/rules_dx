<template>
  <span class="panel">{{ label }}</span>
</template>

<script>
export default {
  props: {
    label: {
      type: String,
      default: "",
    },
  },
};
</script>

<style scoped>
.panel {
  color: black;
}
</style>
