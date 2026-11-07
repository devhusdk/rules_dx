<template>
  <div class="hello">{{ message }}</div>
</template>

<script>
import { helper } from "./helper.js";

export default {
  name: "Hello",
  computed: {
    message() {
      return helper("world");
    },
  },
};
</script>

<style scoped>
.hello {
  color: black;
}
</style>
