import { runCLI } from 'jest';

export function label() {
  if (!runCLI) throw new Error('missing');
  return 'shared-helper';
}
