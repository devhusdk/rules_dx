<script setup lang="ts">
// Ünïcode comment above the props: línea → 日本語 ✓
/** The visible text. */
interface Props {
  /** Text shown inside the button. */
  label: string;
  // Größe der Schaltfläche (non-ASCII line comment)
  /** Optional size in pixels. */
  size?: number;
  /** Whether the button is disabled. */
  disabled?: boolean;
}

const props = withDefaults(defineProps<Props>(), {
  size: 12,
  disabled: false,
});

const emit = defineEmits<{
  /** Fired when the button is clicked. */
  (e: "click", id: number): void;
}>();

defineSlots<{
  /** Content of the button. */
  default(props: { label: string }): unknown;
}>();

/** Moves focus to the button. */
function focus(): void {
  emit("click", props.size);
}

/** The number of clicks seen. */
const clicks = 0;

defineExpose({ focus, clicks });
</script>

<template>
  <button :disabled="props.disabled" @click="focus"><slot :label="props.label" /></button>
</template>
