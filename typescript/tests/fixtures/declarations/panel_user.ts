import type { Panel } from "./panel.js";

export function describe(panel: Panel): string {
	return `panel ${panel.title}`;
}
