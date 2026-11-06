import { load, makeRow } from "../dep/dep";

export function rowId(): number {
  load();
  return makeRow(1).id;
}
