<script lang="ts">
  interface Props {
    title: string;
  }

  let { title }: Props = $props();
  const unused: number = 1;
</script>

<h1 class="title">{@html title}</h1>

<style>
  .title {
    color: black;
  }
</style>
