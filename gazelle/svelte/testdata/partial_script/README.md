# Unclosed script after a valid script

`demo.svelte` has a valid module script, then an unclosed `<script>`. The module script's imports still become deps.
