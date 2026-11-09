/**
 * Greeting helpers.
 * @module greeting
 */

/**
 * Builds a greeting for a name.
 * @param {string} name Who to greet.
 * @returns {string} The greeting text.
 */
export function greet(name) {
  return `Hello, ${name}!`;
}

/** A reusable greeter. */
export class Greeter {
  /**
   * Creates a greeter.
   * @param {string} prefix Text placed before the name.
   */
  constructor(prefix) {
    /** The text placed before the name. */
    this.prefix = prefix;
  }

  /**
   * Greets a name.
   * @param {string} name Who to greet.
   * @returns {string} The greeting text.
   */
  greet(name) {
    return `${this.prefix} ${name}`;
  }
}
