<template>
  <div class="demo">
    <Panel :label="greeting" />
  </div>
</template>

<script>
import helper from "./helper.vue";

export default {
  name: "Demo",
  components: { helper },
};
</script>

<script setup>
import Panel from "./panel.vue";

const greeting = "hello";
</script>

<style scoped>
.demo {
  color: black;
}
</style>
