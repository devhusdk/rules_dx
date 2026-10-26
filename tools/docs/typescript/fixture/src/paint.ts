/** How a shape is filled. */
export interface Paint {
  /** The fill color. */
  color: string;
  /** The stroke width in pixels. */
  stroke?: number;
}

/** The colors every drawing starts with. */
export const Palette: readonly string[] = ["red", "green"];

/** Builds a paint from a color. */
export function paint(color: string, stroke?: number): Paint {
  return { color, stroke };
}
