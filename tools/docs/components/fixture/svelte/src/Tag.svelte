<script lang="ts">
  /** A short tag. */
  interface Props {
    /** Text of the tag. */
    label: string;
  }

  let { label }: Props = $props();
</script>

<span>{label}</span>
