/** A surface that shapes are drawn on. */
export class Canvas {
  /** The width in pixels. */
  readonly width: number;

  /**
   * Creates a canvas.
   * @param width - The width in pixels.
   */
  constructor(width: number) {
    this.width = width;
  }
}

/** Settings that belong to a canvas. */
export namespace Canvas {
  /** How a canvas is created. */
  export type Options = { width: number };
}

/** Draws every shape. */
export function draw(): void {}

/** Settings that belong to drawing. */
export namespace draw {
  /** The default line width. */
  export const lineWidth = 1;
}
