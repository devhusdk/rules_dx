import { widget } from "./widget";
import helper from "./helper";
import fs from "fs";

export function greet(name: string): string {
  return `hello ${name}${helper(fs.sep)}${widget}`;
}
