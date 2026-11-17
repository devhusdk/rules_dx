import { type Io, run, USAGE } from "./cli.js";

declare function test(name: string, fn: () => void): void;
declare const expect: (actual: unknown) => {
	toBe(expected: unknown): void;
	toEqual(expected: unknown): void;
};

const PACKAGES = JSON.stringify([
	{ name: "core", versions: ["1.0.0", "1.1.3", "1.2.0-beta.1"] },
	{ name: "edge", versions: ["0.1.0-alpha.1"] },
]);

interface Run {
	code: number;
	out: string[];
	err: string[];
	read: string[];
}

function exec(
	argv: string[],
	text: string | Error = PACKAGES,
	env?: string,
): Run {
	const result: Run = { code: 0, out: [], err: [], read: [] };
	const io: Io = {
		out: (line) => result.out.push(line),
		err: (line) => result.err.push(line),
		read: (path) => {
			result.read.push(path);
			if (text instanceof Error) {
				throw text;
			}
			return text;
		},
		resolve: (path) => `/work/${path}`,
	};
	result.code = run(argv, io, "/data/packages.json", env);
	return result;
}

test("list prints each package with its latest stable version", () => {
	const result = exec(["list"]);
	expect(result.code).toBe(0);
	expect(result.out).toEqual(["core 1.1.3", "edge none"]);
	expect(result.read).toEqual(["/work//data/packages.json"]);
});

test("--file beats the environment and the packaged file", () => {
	const result = exec(["--file", "mine.json", "list"], PACKAGES, "env.json");
	expect(result.read).toEqual(["/work/mine.json"]);
	expect(exec(["list"], PACKAGES, "env.json").read).toEqual(["/work/env.json"]);
});

test("latest, next and satisfy answer from the registry", () => {
	expect(exec(["latest", "core"]).out).toEqual(["1.1.3"]);
	expect(exec(["next", "core", "minor"]).out).toEqual(["1.2.0"]);
	expect(exec(["satisfy", "core", "^1.0.0"]).out).toEqual(["1.1.3"]);
});

test("usage mistakes exit 2", () => {
	for (const argv of [
		[],
		["sell"],
		["--file"],
		["latest"],
		["next", "core", "huge"],
		["list", "x"],
	]) {
		const result = exec(argv);
		expect(result.code).toBe(2);
		expect(result.err).toEqual([USAGE]);
	}
	expect(exec(["satisfy", "core", "not a range"]).code).toBe(2);
});

test("missing data exits 3 or 4", () => {
	expect(exec(["list"], new Error("gone")).code).toBe(3);
	expect(exec(["list"], "nope").err).toEqual([
		"registry: registry file is not valid JSON",
	]);
	expect(exec(["latest", "absent"]).err).toEqual([
		"registry: no package absent",
	]);
	expect(exec(["latest", "absent"]).code).toBe(4);
	expect(exec(["latest", "edge"]).code).toBe(4);
	expect(exec(["next", "edge", "patch"]).code).toBe(4);
	expect(exec(["satisfy", "core", ">=9.0.0"]).code).toBe(4);
});

test("schema needs no registry file", () => {
	const result = exec(["schema"], new Error("never read"));
	expect(result.out).toEqual(["schema 1"]);
	expect(result.read).toEqual([]);
});
