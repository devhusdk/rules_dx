import { runCLI } from 'jest';

export function hello(name) {
  assertCLI();
  return `hello ${name}`;
}

function assertCLI() {
  if (!runCLI) throw new Error('missing jest');
}
