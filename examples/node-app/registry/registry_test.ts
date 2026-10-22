import {
	type Entry,
	findEntry,
	latestStable,
	nextVersion,
	parseRegistry,
	type RegistryError,
	satisfying,
	schema,
	sortedVersions,
} from "./registry.js";

declare function test(name: string, fn: () => void): void;
declare const expect: (actual: unknown) => {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
	toBeUndefined(): void;
	toThrow(expected?: unknown): void;
};

const core: Entry = {
	name: "core",
	versions: ["1.0.0", "1.1.0", "1.2.0-beta.1", "1.1.3", "0.9.0"],
};

function failure(text: string): RegistryError {
	try {
		parseRegistry(text);
	} catch (error) {
		return error as RegistryError;
	}
	throw new Error("parseRegistry accepted the input");
}

test("sorts by precedence, not by text", () => {
	expect(
		sortedVersions({ name: "x", versions: ["1.10.0", "1.9.0", "1.2.0"] }),
	).toEqual(["1.2.0", "1.9.0", "1.10.0"]);
	expect(sortedVersions(core)).toEqual([
		"0.9.0",
		"1.0.0",
		"1.1.0",
		"1.1.3",
		"1.2.0-beta.1",
	]);
});

test("latest ignores prereleases", () => {
	expect(latestStable(core)).toBe("1.1.3");
	expect(
		latestStable({ name: "edge", versions: ["0.1.0-alpha.1"] }),
	).toBeUndefined();
});

test("next bumps the latest stable version", () => {
	expect(nextVersion(core, "patch")).toBe("1.1.4");
	expect(nextVersion(core, "minor")).toBe("1.2.0");
	expect(nextVersion(core, "major")).toBe("2.0.0");
});

test("next refuses a package without a stable release", () => {
	const edge: Entry = { name: "edge", versions: ["0.1.0-alpha.1"] };
	expect(() => nextVersion(edge, "patch")).toThrow("has no stable release");
});

test("satisfying picks the highest match", () => {
	expect(satisfying(core, "^1.0.0")).toBe("1.1.3");
	expect(satisfying(core, "~1.1.0")).toBe("1.1.3");
	expect(satisfying(core, ">=2.0.0")).toBeUndefined();
	expect(() => satisfying(core, "not a range")).toThrow("is not a valid range");
});

test("parses the packaged shape and finds entries", () => {
	const entries = parseRegistry('[{"name":"a","versions":["1.0.0"]}]');
	expect(findEntry(entries, "a")).toEqual({ name: "a", versions: ["1.0.0"] });
	expect(findEntry(entries, "b")).toBeUndefined();
});

test("rejects malformed registries with a code", () => {
	expect(failure("this is not json {").code).toBe("not_json");
	expect(failure("{}").code).toBe("not_array");
	expect(failure('[{"name":"a"}]').code).toBe("bad_entry");
	expect(failure('[{"name":"","versions":[]}]').code).toBe("bad_entry");
	expect(failure("[null]").code).toBe("bad_entry");
	expect(
		failure('[{"name":"a","versions":[]},{"name":"a","versions":[]}]').message,
	).toBe("duplicate package a");
	expect(failure('[{"name":"a","versions":["v1.0"]}]').message).toBe(
		"package a has an invalid version v1.0",
	);
	expect(failure('[{"name":"a","versions":[" 1.0.0"]}]').code).toBe(
		"bad_version",
	);
	expect(failure('[{"name":"a","versions":[1]}]').code).toBe("bad_version");
});

test("reads the generated schema number", () => {
	expect(schema()).toBe(1);
});
