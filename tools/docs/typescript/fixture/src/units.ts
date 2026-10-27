/** A position on the plane. */
export type Point = { x: number; y: number };

/** The unit a length is measured in. */
export enum Unit {
  /** One millimeter. */
  Millimeter = "mm",
  /** One centimeter. */
  Centimeter = "cm",
}

/**
 * Multiplies a length.
 * @param length - The length to scale.
 * @param by - The factor.
 * @returns The scaled length.
 * @example
 * ```ts
 * scale(2, 3); // 6
 * ```
 */
export function scale(length: number, by: number): number;
/**
 * Doubles a length.
 * @param length - The length to double.
 * @returns The doubled length.
 */
export function scale(length: number): number;
export function scale(length: number, by = 2): number {
  return length * by;
}
