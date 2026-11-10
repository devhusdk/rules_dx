"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const GLIBC =
	/^(ld-linux-.*\.so\.\d+|lib(c|m|dl|pthread|rt|resolv|util)\.so\.\d+)$/;
const DT_NEEDED = 1n;
const DT_STRTAB = 5n;
const PT_LOAD = 1;
const PT_DYNAMIC = 2;

function needed(file) {
	const bytes = fs.readFileSync(file);
	assert.equal(bytes.readUInt32BE(0), 0x7f454c46, `${file} is an ELF file`);
	assert.equal(bytes[4], 2, `${file} is 64-bit`);
	assert.equal(bytes[5], 1, `${file} is little-endian`);
	const phoff = Number(bytes.readBigUInt64LE(32));
	const phentsize = bytes.readUInt16LE(54);
	const phnum = bytes.readUInt16LE(56);
	const loads = [];
	let dynamic = null;
	for (let i = 0; i < phnum; i++) {
		const at = phoff + i * phentsize;
		const segment = {
			type: bytes.readUInt32LE(at),
			offset: bytes.readBigUInt64LE(at + 8),
			vaddr: bytes.readBigUInt64LE(at + 16),
			filesz: bytes.readBigUInt64LE(at + 32),
		};
		if (segment.type === PT_LOAD) {
			loads.push(segment);
		}
		if (segment.type === PT_DYNAMIC) {
			dynamic = segment;
		}
	}
	if (dynamic === null) {
		return [];
	}
	const entries = [];
	for (
		let at = Number(dynamic.offset);
		at < Number(dynamic.offset + dynamic.filesz);
		at += 16
	) {
		entries.push({
			tag: bytes.readBigInt64LE(at),
			value: bytes.readBigUInt64LE(at + 8),
		});
	}
	const strtab = entries.find((entry) => entry.tag === DT_STRTAB).value;
	const load = loads.find(
		(segment) =>
			strtab >= segment.vaddr && strtab < segment.vaddr + segment.filesz,
	);
	const strings = Number(strtab - load.vaddr + load.offset);
	return entries
		.filter((entry) => entry.tag === DT_NEEDED)
		.map((entry) => {
			const start = strings + Number(entry.value);
			return bytes.toString("latin1", start, bytes.indexOf(0, start));
		});
}

const runfiles = process.env.JS_BINARY__RUNFILES || process.env.RUNFILES_DIR;
const marker = path.resolve(runfiles, process.env.DX_PLAYWRIGHT_MARKER);
const browsers = path.dirname(marker);
const closure = path.join(
	path.dirname(path.dirname(browsers)),
	process.env.DX_PLAYWRIGHT_CLOSURE_DIR,
	"lib",
);
const shipped = new Set(fs.readdirSync(closure));

const [install] = fs
	.readdirSync(browsers)
	.filter((entry) => entry.startsWith("chrome-headless-shell-"));
const shell = path.join(browsers, install, "chrome-headless-shell");
const bundled = fs
	.readdirSync(path.join(browsers, install))
	.filter((entry) => /\.so(\.\d+)*$/.test(entry));
const subjects = [shell]
	.concat(bundled.map((entry) => path.join(browsers, install, entry)))
	.concat([...shipped].map((entry) => path.join(closure, entry)));

const roots = needed(shell);
assert.ok(
	roots.includes("libnss3.so"),
	"the parser reads the browser's needed libraries",
);
assert.ok(
	shipped.has("libsoftokn3.so"),
	"the closure ships the NSS modules loaded at run time",
);

const outside = [];
for (const subject of subjects) {
	for (const library of needed(subject)) {
		if (!GLIBC.test(library) && !shipped.has(library)) {
			outside.push(`${path.basename(subject)} needs ${library}`);
		}
	}
}
assert.deepEqual(outside, []);
