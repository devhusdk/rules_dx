import type { Property } from "csstype";

/** How a shape is filled. */
export interface Paint {
  /** The fill color. */
  color: Property.Color;
  /** The stroke width in pixels. */
  stroke?: number;
}

/** The colors every drawing starts with. */
export const Palette: readonly Property.Color[] = ["red", "green"];

/** Builds a paint from a color. */
export function paint(color: Property.Color, stroke?: number): Paint {
  return { color, stroke };
}
