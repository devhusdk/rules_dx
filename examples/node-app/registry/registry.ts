import semver from "semver";
import { SCHEMA } from "./build_info.js";

export type Bump = "major" | "minor" | "patch";

export type ErrorCode =
	| "not_json"
	| "not_array"
	| "bad_entry"
	| "bad_version"
	| "bad_range"
	| "no_stable";

export interface Entry {
	name: string;
	versions: string[];
}

export class RegistryError extends Error {
	readonly code: ErrorCode;

	constructor(code: ErrorCode, message: string) {
		super(message);
		this.code = code;
	}
}

export function schema(): number {
	return SCHEMA;
}

export function parseRegistry(text: string): Entry[] {
	let value: unknown;
	try {
		value = JSON.parse(text);
	} catch {
		throw new RegistryError("not_json", "registry file is not valid JSON");
	}
	if (!Array.isArray(value)) {
		throw new RegistryError("not_array", "registry file must hold an array");
	}
	const seen = new Set<string>();
	return value.map((raw: unknown): Entry => {
		const entry = raw as Partial<Entry> | null;
		if (
			typeof entry?.name !== "string" ||
			entry.name === "" ||
			!Array.isArray(entry.versions)
		) {
			throw new RegistryError(
				"bad_entry",
				"each package needs a name and a versions array",
			);
		}
		if (seen.has(entry.name)) {
			throw new RegistryError("bad_entry", `duplicate package ${entry.name}`);
		}
		seen.add(entry.name);
		for (const version of entry.versions) {
			if (typeof version !== "string" || semver.valid(version) !== version) {
				throw new RegistryError(
					"bad_version",
					`package ${entry.name} has an invalid version ${String(version)}`,
				);
			}
		}
		return { name: entry.name, versions: [...entry.versions] };
	});
}

export function findEntry(entries: Entry[], name: string): Entry | undefined {
	return entries.find((entry) => entry.name === name);
}

export function sortedVersions(entry: Entry): string[] {
	return [...entry.versions].sort(semver.compare);
}

export function latestStable(entry: Entry): string | undefined {
	const stable = sortedVersions(entry).filter(
		(version) => semver.prerelease(version) === null,
	);
	return stable.pop();
}

export function nextVersion(entry: Entry, bump: Bump): string {
	const current = latestStable(entry);
	if (current === undefined) {
		throw new RegistryError(
			"no_stable",
			`package ${entry.name} has no stable release`,
		);
	}
	return semver.inc(current, bump) as string;
}

export function satisfying(entry: Entry, range: string): string | undefined {
	if (semver.validRange(range) === null) {
		throw new RegistryError("bad_range", `${range} is not a valid range`);
	}
	return semver.maxSatisfying(entry.versions, range) ?? undefined;
}
