import {
	type Bump,
	type Entry,
	findEntry,
	latestStable,
	nextVersion,
	parseRegistry,
	RegistryError,
	satisfying,
	schema,
} from "../registry/registry.js";

export interface Io {
	out: (line: string) => void;
	err: (line: string) => void;
	read: (path: string) => string;
	resolve: (path: string) => string;
}

export const USAGE =
	"usage: registry [--file PATH] list | latest NAME | next NAME major|minor|patch | satisfy NAME RANGE | schema | where";

const BUMPS: readonly string[] = ["major", "minor", "patch"];

function lookup(entries: Entry[], name: string | undefined): Entry | number {
	const entry = name === undefined ? undefined : findEntry(entries, name);
	return entry ?? 4;
}

export function run(
	argv: string[],
	io: Io,
	defaultFile: string,
	envFile: string | undefined,
): number {
	const args = [...argv];
	let file = envFile ?? defaultFile;
	if (args[0] === "--file") {
		const given = args[1];
		if (given === undefined) {
			io.err(USAGE);
			return 2;
		}
		file = given;
		args.splice(0, 2);
	}
	const command = args.shift();
	if (command === "schema") {
		io.out(`schema ${schema()}`);
		return 0;
	}
	if (
		command === undefined ||
		!["list", "latest", "next", "satisfy"].includes(command)
	) {
		io.err(USAGE);
		return 2;
	}
	const wanted = command === "list" ? 0 : command === "latest" ? 1 : 2;
	if (args.length !== wanted) {
		io.err(USAGE);
		return 2;
	}
	if (command === "next" && !BUMPS.includes(args[1] ?? "")) {
		io.err(USAGE);
		return 2;
	}
	let entries: Entry[];
	try {
		entries = parseRegistry(io.read(io.resolve(file)));
	} catch (error) {
		if (error instanceof RegistryError) {
			io.err(`registry: ${error.message}`);
		} else {
			io.err("registry: cannot read registry file");
		}
		return 3;
	}
	if (command === "list") {
		for (const entry of entries) {
			io.out(`${entry.name} ${latestStable(entry) ?? "none"}`);
		}
		return 0;
	}
	const entry = lookup(entries, args[0]);
	if (typeof entry === "number") {
		io.err(`registry: no package ${args[0]}`);
		return entry;
	}
	try {
		if (command === "latest") {
			const version = latestStable(entry);
			if (version === undefined) {
				io.err(`registry: package ${entry.name} has no stable release`);
				return 4;
			}
			io.out(version);
		} else if (command === "next") {
			io.out(nextVersion(entry, args[1] as Bump));
		} else {
			const range = args[1] as string;
			const version = satisfying(entry, range);
			if (version === undefined) {
				io.err(`registry: no version of ${entry.name} satisfies ${range}`);
				return 4;
			}
			io.out(version);
		}
	} catch (error) {
		if (error instanceof RegistryError) {
			io.err(`registry: ${error.message}`);
			return error.code === "bad_range" ? 2 : 4;
		}
		throw error;
	}
	return 0;
}
