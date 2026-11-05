import { runCLI } from 'jest';

export function hello(name: string): string {
  assertCLI();
  return `hello ${name}`;
}

function assertCLI(): void {
  if (!runCLI) throw new Error('missing jest');
}
