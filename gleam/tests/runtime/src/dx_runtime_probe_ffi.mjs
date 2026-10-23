export function root_dir() {
  return process.execPath;
}

export function runtime() {
  return process.release.name;
}

export function version() {
  return process.versions.node;
}
