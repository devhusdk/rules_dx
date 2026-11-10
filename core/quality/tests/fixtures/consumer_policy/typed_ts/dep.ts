export interface Row {
  id: number;
}

export function makeRow(id: number): Row {
  return { id };
}
