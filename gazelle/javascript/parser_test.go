package javascript

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
		{"sideEffect", "import \"./hello.js\";\n", []string{"hello"}},
		{"sideEffectSingle", "import './helper';\n", []string{"helper"}},
		{"default", "import hello from \"./hello.js\";\n", []string{"hello"}},
		{"named", "import {a, b} from './util.mjs';\n", []string{"util"}},
		{"namespace", "import * as ns from \"../pkg/demo.jsx\";\n", []string{"demo"}},
		{"bare", "import React from \"react\";\n", []string{"react"}},
		{"scoped", "import x from \"@scope/pkg/sub\";\n", []string{"@scope/pkg/sub"}},
		{"scopedRoot", "import x from \"@scope/pkg\";\n", []string{"@scope/pkg"}},
		{"subpath", "import x from \"pkg/subpath\";\n", []string{"pkg/subpath"}},
		{"nodeBuiltin", "import fs from \"fs\";\n", []string{"fs"}},
		{"nodePrefix", "import fs from \"node:fs\";\n", []string{"node:fs"}},
		{"exportNamed", "export {a} from './helper.js';\n", []string{"helper"}},
		{"exportStar", "export * from \"./demo.cjs\";\n", []string{"demo"}},
		{"exportStarAs", "export * as ns from '../lib.js';\n", []string{"lib"}},
		{"exportLocal", "export const x = 1;\n", nil},
		{"dynamic", "const m = await import(\"./lazy.js\");\n", []string{"lazy"}},
		{"dynamicSingle", "import('./other.mjs');\n", []string{"other"}},
		{"dynamicBare", "import(\"react\");\n", []string{"react"}},
		{"require", "const x = require(\"./data.cjs\");\n", []string{"data"}},
		{"requireBare", "require('fs');\n", []string{"fs"}},
		{"requireComputed", "require(name);\n", nil},
		{"dynamicComputed", "import(name);\n", nil},
		{"templateDynamic", "import(`./lazy.js`);\n", nil},
		{"importMeta", "const u = import.meta.url;\n", nil},
		{"methodRequire", "obj.require(\"./fake.js\");\n", nil},
		{"noImports", "const x = 1;\n", nil},
		{"lineComment", "// import foo from \"bar\";\n", nil},
		{"blockComment", "/* import \"./hidden.js\"; */\nconst x = 1;\n", nil},
		{"trailingComment", "import a from \"./real.js\"; // import \"./fake.js\";\n", []string{"real"}},
		{"doubleString", "\"import './fake.js'\";\nimport y from './real.js';\n", []string{"real"}},
		{"singleString", "'require(\"./fake.js\")';\n", nil},
		{"templateInert", "`import './fake.js'`;\n", nil},
		{"regexInert", "const re = /import './fake.js'/;\nconst x = 1;\n", nil},
		{"unterminated", "\"abc\nimport y from './real.js';\n", []string{"real"}},
		{"relativeNoExt", "import x from './helper';\n", []string{"helper"}},
		{"relativeDir", "import x from './dir/';\n", []string{"dir"}},
		{"multiline", "import {\n a,\n b\n} from './multi.js';\n", []string{"multi"}},
		{"exportMultiline", "export {\na\n} from \"./shared.js\";\n", []string{"shared"}},
		{"duplicate", "import a from './same.js';\nimport b from './same.js';\n", []string{"same"}},
		{"emptySpec", "import \"\";\n", nil},
		{"unterminatedBlock", "/* import \"./hidden.js\";", nil},
		{"dynamicMissingParen", "import(\"a\";\n", nil},
		{"dynamicUnterminated", "import(\"abc\n", nil},
		{"dynamicOpenEOF", "import(", nil},
		{"importType", "import type {A} from './types.js';\n", []string{"types"}},
		{"importTypePrefix", "import typeofoo from './x.js';\n", []string{"x"}},
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
		{"exportType", "export type {A} from './types.js';\n", []string{"types"}},
		{"exportStrayString", "export \"x\";\n", nil},
		{"exportTemplate", "export `tmpl`;\n", nil},
		{"exportNewlineFrom", "export {a}\nfrom './helper.js';\n", []string{"helper"}},
		{"exportNewlineNoFrom", "export const x = 1\n", nil},
		{"exportFromUnterminated", "export {a} from \"abc\n", nil},
		{"exportFromNonQuote", "export {a} from bar;\n", nil},
		{"exportEOF", "export", nil},
		{"exportBraceEOF", "export {", nil},
		{"requireNoParen", "require;\n", nil},
		{"requireEOF", "require", nil},
		{"requireMissingParen", "require(\"a\";\n", nil},
		{"requireUnterminated", "require(\"abc\n", nil},
		{"triviaLineComment", "import //c\n\"./a.js\";\n", []string{"a"}},
		{"triviaBlockComment", "import /*c*/ \"./a.js\";\n", []string{"a"}},
		{"triviaUnterminatedBlock", "import /* unterminated", nil},
		{"quotedEscape", "import \"a\\\"b\";\n", []string{"a\"b"}},
		{"quotedBackslashEOF", "import \"abc\\", nil},
		{"quotedEOF", "import \"abc", nil},
		{"templateEscape", "`a\\nb`;\nimport y from './real.js';\n", []string{"real"}},
		{"templateInterp", "`outer ${x} inner`;\nimport y from './real.js';\n", []string{"real"}},
		{"templateUnterminated", "`abc", nil},
		{"regexAtStart", "/abc/;\nimport y from './real.js';\n", []string{"real"}},
		{"division", "const x = a / b;\nimport y from './real.js';\n", []string{"real"}},
		{"regexEscape", "const re = /a\\/b/;\nimport y from './real.js';\n", []string{"real"}},
		{"regexNewline", "const re = /abc\nimport y from './real.js';\n", []string{"real"}},
		{"regexClass", "const re = /[a/b]/;\nimport y from './real.js';\n", []string{"real"}},
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
	for _, name := range []string{"helper", "", "fs-extra", "react", "fs/promises/extra"} {
		if IsStdLib(name) {
			t.Errorf("non-stdlib identity recognized as stdlib: %q", name)
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
		{"relativeStdlibCollision", "import { fmt } from \"./util.js\";\n", []ImportRef{{Root: "util", Relative: true}}},
		{"bareStdlib", "import fs from \"fs\";\n", []ImportRef{{Root: "fs", Relative: false}}},
		{"bareNonStdlib", "import React from \"react\";\n", []ImportRef{{Root: "react", Relative: false}}},
		{"mixedCollision", "import { fmt } from \"./util.js\";\nimport u from \"util\";\n", []ImportRef{{Root: "util", Relative: true}}},
		{"absolute", "import x from \"/abs/path.js\";\n", []ImportRef{{Root: "path", Relative: true}}},
	}
	for _, tc := range cases {
		if got := ParseImportRefs([]byte(tc.source)); !reflect.DeepEqual(got, tc.want) {
			t.Errorf("%s: ParseImportRefs = %+v, want %+v", tc.name, got, tc.want)
		}
	}
	for _, spec := range []string{"./x.js", "../y.js", "/z.js", " ./w.js "} {
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
