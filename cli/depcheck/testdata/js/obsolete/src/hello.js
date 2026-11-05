import { runCLI } from 'jest';

export function hello(name) {
  if (!runCLI) throw new Error('missing');
  return `hello ${name}`;
}
