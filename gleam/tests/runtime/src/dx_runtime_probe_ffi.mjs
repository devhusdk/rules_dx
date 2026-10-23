import { existsSync } from "node:fs";

export function root_dir() {
  return process.execPath;
}

export function runtime() {
  return process.release.name;
}

export function version() {
  return process.versions.node;
}

export function file_exists(path) {
  return existsSync(path);
}
