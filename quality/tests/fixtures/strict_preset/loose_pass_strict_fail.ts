// Loose passes, strict fails example.
//
// Implicit-any `name` passes with strict false and fails with
// strict true (noImplicitAny); explicit `any` is the Biome strict
// `noExplicitAny` illustration. Illustrative only; not executed by the
// qualification harness as a tool run.
export function greet(name) {
	const result: any = name;
	return result;
}
