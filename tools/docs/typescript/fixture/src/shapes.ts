import type { Point } from "./units";

/** A closed two-dimensional figure. */
export interface Shape {
  /** Where the figure is centered. */
  readonly center: Point;
  /** Returns the enclosed area. */
  area(): number;
}

/** A circle around a center point. */
export class Circle implements Shape {
  /** Where the circle is centered. */
  readonly center: Point;

  /**
   * Creates a circle.
   * @param center - The middle of the circle.
   * @param radius - The distance from the middle to the edge.
   */
  constructor(
    center: Point,
    public radius: number,
  ) {
    this.center = center;
  }

  /** Returns the enclosed area. */
  area(): number {
    return Math.PI * this.radius ** 2;
  }

  /** Returns a copy with the radius multiplied by `factor`. */
  grow(factor = 2): Circle {
    return new Circle(this.center, this.radius * factor);
  }

  /** @internal */
  debugName(): string {
    return "circle";
  }

  private secret(): number {
    return 1;
  }
}
