<script lang="ts">
  import logo from "../assets/logo.png";

  let { title }: { title: string } = $props();
</script>

<header class="site-header">
  <img src={logo} alt="logo" />
  <h1>{title}</h1>
</header>
