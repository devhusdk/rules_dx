import { runCLI } from 'jest';
import { check } from 'test-helper';

export function hello(name) {
  if (!runCLI) throw new Error('missing');
  if (!check(name)) throw new Error('bad');
  return `hello ${name}`;
}
