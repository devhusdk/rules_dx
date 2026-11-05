import { runCLI } from 'jest';
import optionalFeat from 'optional-feat';

export function hello(name) {
  if (!runCLI) throw new Error('missing');
  return `hello ${name} ${optionalFeat}`;
}
