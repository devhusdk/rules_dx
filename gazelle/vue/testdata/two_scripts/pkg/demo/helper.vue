<template>
  <span class="helper">{{ label }}</span>
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
.helper {
  color: black;
}
</style>
