import { runCLI } from 'jest';

export function hello(name) {
  if (!runCLI) throw new Error('missing');
  return `hello ${name}`;
}

// build-plugin runs via jest config (no import by design).
export function version() {
  return '0.0.0';
}
