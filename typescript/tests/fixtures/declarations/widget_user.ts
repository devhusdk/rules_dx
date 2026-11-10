import type { Widget } from "./widget.js";

export function widgetKind(value: Widget): string {
	return value.kind;
}
