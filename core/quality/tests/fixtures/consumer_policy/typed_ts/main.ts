import { makeRow, type Row } from "./dep";

export function rowId(): number {
  const row: Row = makeRow(1);
  return row.id;
}
