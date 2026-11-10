export interface Row {
  id: number;
}

export function makeRow(id: number): Row {
  return { id };
}

export function load(): Promise<Row> {
  return Promise.resolve(makeRow(0));
}
