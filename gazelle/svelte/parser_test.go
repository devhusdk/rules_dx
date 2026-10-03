package svelte

import (
	"reflect"
	"testing"
)

func wrap(script string) string {
	return "<script>\n" + script + "</script>\n\n<div class=\"hello\">hello</div>\n\n<style>\n.hello {\n  color: black;\n}\n</style>\n"
}

func wrapModule(module, instance string) string {
	return "<script context=\"module\">\n" + module + "</script>\n\n<script>\n" + instance + "</script>\n\n<div class=\"hello\">hello</div>\n"
}

func TestParseImports(t *testing.T) {
	cases := []struct {
		name   string
		source string
		want   []string
	}{
		{"empty", "", nil},
		{"noScript", "<div>hello</div>\n", nil},
		{"emptyScript", "<script></script>\n", nil},
		{"selfClosingScript", "<script/>\n", nil},
		{"markupOnlyImport", "<div>{import \"./fake.svelte\"}</div>\n", nil},
		{"styleOnlyImport", "<style>@import \"./fake.css\";</style>\n", nil},
		{"markupStyleInert", "<div>import \"./fake.svelte\"</div>\n<script>\nimport y from './real.svelte';\n</script>\n<style>import \"./other.svelte\"</style>\n", []string{"real"}},
		{"moduleAndInstance", wrapModule("import a from './mod.svelte';\n", "import b from './inst.svelte';\n"), []string{"inst", "mod"}},
		{"moduleOnly", "<script context=\"module\">\nimport a from './mod.svelte';\n</script>\n<div>x</div>\n", []string{"mod"}},
		{"duplicateAcrossBlocks", wrapModule("import a from './same.svelte';\n", "import b from './same.svelte';\n"), []string{"same"}},
		{"unparseableSecondInert", "<script>\nimport a from './a.svelte';\n</script>\n<script>const x = 1;", nil},
		{"sideEffect", "import \"./hello.svelte\";\n", []string{"hello"}},
		{"sideEffectSingle", "import './helper';\n", []string{"helper"}},
		{"default", "import hello from \"./hello.svelte\";\n", []string{"hello"}},
		{"named", "import {a, b} from './util.mjs';\n", []string{"util"}},
		{"namespace", "import * as ns from \"../pkg/demo.svelte\";\n", []string{"demo"}},
		{"bare", "import React from \"react\";\n", []string{"react"}},
		{"svelteBare", "import { onMount } from \"svelte\";\n", []string{"svelte"}},
		{"svelteCompiler", "import { parse } from \"svelte/compiler\";\n", []string{"svelte/compiler"}},
		{"scoped", "import x from \"@scope/pkg/sub\";\n", []string{"@scope/pkg/sub"}},
		{"scopedRoot", "import x from \"@scope/pkg\";\n", []string{"@scope/pkg"}},
		{"subpath", "import x from \"pkg/subpath\";\n", []string{"pkg/subpath"}},
		{"nodeBuiltin", "import fs from \"fs\";\n", []string{"fs"}},
		{"nodePrefix", "import fs from \"node:fs\";\n", []string{"node:fs"}},
		{"exportNamed", "export {a} from './helper.svelte';\n", []string{"helper"}},
		{"exportStar", "export * from \"./demo.svelte\";\n", []string{"demo"}},
		{"exportStarAs", "export * as ns from '../lib.svelte';\n", []string{"lib"}},
		{"exportLocal", "export const x = 1;\n", nil},
		{"dynamic", "const m = await import(\"./lazy.svelte\");\n", []string{"lazy"}},
		{"dynamicSingle", "import('./other.mjs');\n", []string{"other"}},
		{"dynamicBare", "import(\"react\");\n", []string{"react"}},
		{"require", "const x = require(\"./data.cjs\");\n", []string{"data"}},
		{"requireBare", "require('fs');\n", []string{"fs"}},
		{"requireComputed", "require(name);\n", nil},
		{"dynamicComputed", "import(name);\n", nil},
		{"templateDynamic", "import(`./lazy.svelte`);\n", nil},
		{"importMeta", "const u = import.meta.url;\n", nil},
		{"methodRequire", "obj.require(\"./fake.svelte\");\n", nil},
		{"noImports", "const x = 1;\n", nil},
		{"lineComment", "// import foo from \"bar\";\n", nil},
		{"blockComment", "/* import \"./hidden.svelte\"; */\nconst x = 1;\n", nil},
		{"trailingComment", "import a from \"./real.svelte\"; // import \"./fake.svelte\";\n", []string{"real"}},
		{"doubleString", "\"import './fake.svelte'\";\nimport y from './real.svelte';\n", []string{"real"}},
		{"singleString", "'require(\"./fake.svelte\")';\n", nil},
		{"templateInert", "`import './fake.svelte'`;\n", nil},
		{"regexInert", "const re = /import './fake.svelte'/;\nconst x = 1;\n", nil},
		{"unterminated", "\"abc\nimport y from './real.svelte';\n", []string{"real"}},
		{"relativeNoExt", "import x from './helper';\n", []string{"helper"}},
		{"relativeDir", "import x from './dir/';\n", []string{"dir"}},
		{"multiline", "import {\n a,\n b\n} from './multi.svelte';\n", []string{"multi"}},
		{"exportMultiline", "export {\na\n} from \"./shared.svelte\";\n", []string{"shared"}},
		{"duplicate", "import a from './same.svelte';\nimport b from './same.svelte';\n", []string{"same"}},
		{"emptySpec", "import \"\";\n", nil},
		{"unterminatedBlock", "/* import \"./hidden.svelte\";", nil},
		{"dynamicMissingParen", "import(\"a\";\n", nil},
		{"dynamicUnterminated", "import(\"abc\n", nil},
		{"dynamicOpenEOF", "import(", nil},
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
		{"exportNewlineFrom", "export {a}\nfrom './helper.svelte';\n", []string{"helper"}},
		{"exportNewlineNoFrom", "export const x = 1\n", nil},
		{"exportFromUnterminated", "export {a} from \"abc\n", nil},
		{"exportFromNonQuote", "export {a} from bar;\n", nil},
		{"exportEOF", "export", nil},
		{"exportBraceEOF", "export {", nil},
		{"requireNoParen", "require;\n", nil},
		{"requireEOF", "require", nil},
		{"requireMissingParen", "require(\"a\";\n", nil},
		{"requireUnterminated", "require(\"abc\n", nil},
		{"triviaLineComment", "import //c\n\"./a.svelte\";\n", []string{"a"}},
		{"triviaBlockComment", "import /*c*/ \"./a.svelte\";\n", []string{"a"}},
		{"triviaUnterminatedBlock", "import /* unterminated", nil},
		{"importBraceComment", "import {a /*c*/} from './real.svelte';\n", []string{"real"}},
		{"exportBraceComment", "export {a /*c*/} from './real.svelte';\n", []string{"real"}},
		{"inertEscape", "\"a\\nb\";\nimport y from './real.svelte';\n", []string{"real"}},
		{"templateInterpObject", "`outer ${ {a: 1} } inner`;\nimport y from './real.svelte';\n", []string{"real"}},
		{"quotedEscape", "import \"a\\\"b\";\n", []string{"a\"b"}},
		{"quotedBackslashEOF", "import \"abc\\", nil},
		{"quotedEOF", "import \"abc", nil},
		{"templateEscape", "`a\\nb`;\nimport y from './real.svelte';\n", []string{"real"}},
		{"templateInterp", "`outer ${x} inner`;\nimport y from './real.svelte';\n", []string{"real"}},
		{"templateInterpQuote", "`outer ${'x'} inner`;\nimport y from './real.svelte';\n", []string{"real"}},
		{"templateInterpNested", "`outer ${`inner`} end`;\nimport y from './real.svelte';\n", []string{"real"}},
		{"templateUnterminated", "`abc", nil},
		{"regexAtStart", "/abc/;\nimport y from './real.svelte';\n", []string{"real"}},
		{"division", "const x = a / b;\nimport y from './real.svelte';\n", []string{"real"}},
		{"divisionParen", "const x = (a) / b;\nimport y from './real.svelte';\n", []string{"real"}},
		{"regexEscape", "const re = /a\\/b/;\nimport y from './real.svelte';\n", []string{"real"}},
		{"regexNewline", "const re = /abc\nimport y from './real.svelte';\n", []string{"real"}},
		{"regexClass", "const re = /[a/b]/;\nimport y from './real.svelte';\n", []string{"real"}},
		{"regexEOF", "const re = /abc", nil},
	}
	for _, tc := range cases {
		var content []byte
		switch tc.name {
		case "empty", "noScript", "emptyScript", "selfClosingScript",
			"markupOnlyImport", "styleOnlyImport", "markupStyleInert",
			"moduleAndInstance", "moduleOnly", "duplicateAcrossBlocks",
			"unparseableSecondInert":
			content = []byte(tc.source)
		default:
			content = []byte(wrap(tc.source))
		}
		if got := ParseImports(content); !reflect.DeepEqual(got, tc.want) {
			t.Errorf("%s: ParseImports = %q, want %q", tc.name, got, tc.want)
		}
	}
}
