import { runCLI } from 'jest';
import { check } from 'test-helper';
import { hello } from '../src/hello.js';

export function testHello() {
  if (!runCLI) throw new Error('missing');
  if (!check('world')) throw new Error('bad');
  return hello('world');
}
