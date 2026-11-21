import { defineStore } from "pinia";
import { ref } from "vue";

export const useCountStore = defineStore("count", () => {
	const total = ref(0);
	function record(value: number): void {
		total.value = value;
	}
	return { total, record };
});
