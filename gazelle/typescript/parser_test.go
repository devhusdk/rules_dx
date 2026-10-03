package typescript

import (
	"reflect"
	"testing"
)

func TestParseImports(t *testing.T) {
	cases := []struct {
		name   string
		source string
		want   []string
	}{
		{"empty", "", nil},
		{"sideEffect", "import \"./hello.ts\";\n", []string{"hello"}},
		{"default", "import hello from \"./hello.ts\";\n", []string{"hello"}},
		{"named", "import {a} from './util.tsx';\n", []string{"util"}},
		{"typeImport", "import type {A} from './types.ts';\n", []string{"types"}},
		{"bare", "import React from \"react\";\n", []string{"react"}},
		{"nodeBuiltin", "import fs from \"fs\";\n", []string{"fs"}},
		{"exportNamed", "export {a} from './helper.ts';\n", []string{"helper"}},
		{"exportType", "export type {A} from \"./types.mts\";\n", []string{"types"}},
		{"exportStar", "export * from \"./demo.cts\";\n", []string{"demo"}},
		{"exportLocal", "export interface A { x: number }\n", nil},
		{"exportDecl", "export declare const x: number;\n", nil},
		{"dynamic", "const m = await import(\"./lazy.ts\");\n", []string{"lazy"}},
		{"require", "const x = require(\"./data.cjs\");\n", []string{"data"}},
		{"importMeta", "const u = import.meta.url;\n", nil},
		{"lineComment", "// import foo from \"bar\";\n", nil},
		{"relativeNoExt", "import x from './helper';\n", []string{"helper"}},
		{"duplicate", "import a from './same.ts';\nimport b from './same.ts';\n", []string{"same"}},
		{"multilineExport", "export {\na\n} from \"./shared.ts\";\n", []string{"shared"}},
		{"sideEffectSingle", "import './helper';\n", []string{"helper"}},
		{"namespace", "import * as ns from \"../pkg/demo.tsx\";\n", []string{"demo"}},
		{"scoped", "import x from \"@scope/pkg/sub\";\n", []string{"@scope/pkg/sub"}},
		{"scopedRoot", "import x from \"@scope/pkg\";\n", []string{"@scope/pkg"}},
		{"subpath", "import x from \"pkg/subpath\";\n", []string{"pkg/subpath"}},
		{"nodePrefix", "import fs from \"node:fs\";\n", []string{"node:fs"}},
		{"exportStarAs", "export * as ns from '../lib.ts';\n", []string{"lib"}},
		{"dynamicSingle", "import('./other.mts');\n", []string{"other"}},
		{"dynamicBare", "import(\"react\");\n", []string{"react"}},
		{"requireBare", "require('fs');\n", []string{"fs"}},
		{"requireComputed", "require(name);\n", nil},
		{"dynamicComputed", "import(name);\n", nil},
		{"templateDynamic", "import(`./lazy.ts`);\n", nil},
		{"methodRequire", "obj.require(\"./fake.ts\");\n", nil},
		{"noImports", "const x = 1;\n", nil},
		{"blockComment", "/* import \"./hidden.ts\"; */\nconst x = 1;\n", nil},
		{"trailingComment", "import a from \"./real.ts\"; // import \"./fake.ts\";\n", []string{"real"}},
		{"doubleString", "\"import './fake.ts'\";\nimport y from './real.ts';\n", []string{"real"}},
		{"singleString", "'require(\"./fake.ts\")';\n", nil},
		{"templateInert", "`import './fake.ts'`;\n", nil},
		{"regexInert", "const re = /import './fake.ts'/;\nconst x = 1;\n", nil},
		{"unterminated", "\"abc\nimport y from './real.ts';\n", []string{"real"}},
		{"relativeDir", "import x from './dir/';\n", []string{"dir"}},
		{"multiline", "import {\n a,\n b\n} from './multi.ts';\n", []string{"multi"}},
		{"emptySpec", "import \"\";\n", nil},
		{"unterminatedBlock", "/* import \"./hidden.ts\";", nil},
		{"dynamicMissingParen", "import(\"a\";\n", nil},
		{"dynamicUnterminated", "import(\"abc\n", nil},
		{"dynamicOpenEOF", "import(", nil},
		{"importTypePrefix", "import typeofoo from './x.ts';\n", []string{"x"}},
		{"importTypeEOF", "import type", nil},
		{"importShortTail", "import typ", nil},
		{"importBareEOF", "import", nil},
		{"sideEffectUnterminated", "import \"abc\n", nil},
		{"staticStrayString", "import foo \"bar\";\n", nil},
		{"staticTemplate", "import x `tmpl`;\n", nil},
		{"staticSemicolon", "import foo;\n", nil},
		{"staticEOF", "import foo", nil},
		{"fromUnterminated", "import foo from \"abc\n", nil},
		{"fromNonQuote", "import foo from bar;\n", nil},
		{"exportStrayString", "export \"x\";\n", nil},
		{"exportTemplate", "export `tmpl`;\n", nil},
		{"exportNewlineFrom", "export {a}\nfrom './helper.ts';\n", []string{"helper"}},
		{"exportNewlineNoFrom", "export const x = 1\n", nil},
		{"exportFromUnterminated", "export {a} from \"abc\n", nil},
		{"exportFromNonQuote", "export {a} from bar;\n", nil},
		{"exportEOF", "export", nil},
		{"exportBraceEOF", "export {", nil},
		{"requireNoParen", "require;\n", nil},
		{"requireEOF", "require", nil},
		{"requireMissingParen", "require(\"a\";\n", nil},
		{"requireUnterminated", "require(\"abc\n", nil},
		{"triviaLineComment", "import //c\n\"./a.ts\";\n", []string{"a"}},
		{"triviaBlockComment", "import /*c*/ \"./a.ts\";\n", []string{"a"}},
		{"triviaUnterminatedBlock", "import /* unterminated", nil},
		{"quotedEscape", "import \"a\\\"b\";\n", []string{"a\"b"}},
		{"quotedBackslashEOF", "import \"abc\\", nil},
		{"quotedEOF", "import \"abc", nil},
		{"templateEscape", "`a\\nb`;\nimport y from './real.ts';\n", []string{"real"}},
		{"templateInterp", "`outer ${x} inner`;\nimport y from './real.ts';\n", []string{"real"}},
		{"templateUnterminated", "`abc", nil},
		{"regexAtStart", "/abc/;\nimport y from './real.ts';\n", []string{"real"}},
		{"division", "const x = a / b;\nimport y from './real.ts';\n", []string{"real"}},
		{"regexEscape", "const re = /a\\/b/;\nimport y from './real.ts';\n", []string{"real"}},
		{"regexNewline", "const re = /abc\nimport y from './real.ts';\n", []string{"real"}},
		{"regexClass", "const re = /[a/b]/;\nimport y from './real.ts';\n", []string{"real"}},
		{"regexEOF", "const re = /abc", nil},
	}
	for _, tc := range cases {
		if got := ParseImports([]byte(tc.source)); !reflect.DeepEqual(got, tc.want) {
			t.Errorf("%s: ParseImports = %q, want %q", tc.name, got, tc.want)
		}
	}
}

func TestIsStdLib(t *testing.T) {
	for _, name := range []string{"fs", "path", "node:fs", "node:path", "test", "node:test"} {
		if !IsStdLib(name) {
			t.Errorf("stdlib identity not recognized: %q", name)
		}
	}
	for _, name := range []string{"helper", "", "react", "fs-extra", "fs/promises/extra"} {
		if IsStdLib(name) {
			t.Errorf("non-stdlib recognized as stdlib: %q", name)
		}
	}
	if !IsStdLib("fs/promises") {
		t.Error("stdlib subpath fs/promises not recognized")
	}
}

func TestParseImportRefsRelative(t *testing.T) {
	cases := []struct {
		name   string
		source string
		want   []ImportRef
	}{
		{"relativeStdlibCollision", "import { add } from \"./util.js\";\n", []ImportRef{{Root: "util", Relative: true}}},
		{"bareStdlib", "import fs from \"fs\";\n", []ImportRef{{Root: "fs", Relative: false}}},
		{"bareNonStdlib", "import React from \"react\";\n", []ImportRef{{Root: "react", Relative: false}}},
		{"mixedCollision", "import { add } from \"./util.js\";\nimport u from \"util\";\n", []ImportRef{{Root: "util", Relative: true}}},
		{"absolute", "import x from \"/abs/path.ts\";\n", []ImportRef{{Root: "path", Relative: true}}},
	}
	for _, tc := range cases {
		if got := ParseImportRefs([]byte(tc.source)); !reflect.DeepEqual(got, tc.want) {
			t.Errorf("%s: ParseImportRefs = %+v, want %+v", tc.name, got, tc.want)
		}
	}
	for _, spec := range []string{"./x.ts", "../y.ts", "/z.ts", " ./w.ts "} {
		if !IsRelativeSpec(spec) {
			t.Errorf("IsRelativeSpec(%q) = false, want true", spec)
		}
	}
	for _, spec := range []string{"react", "fs", "node:fs", ""} {
		if IsRelativeSpec(spec) {
			t.Errorf("IsRelativeSpec(%q) = true, want false", spec)
		}
	}
}
