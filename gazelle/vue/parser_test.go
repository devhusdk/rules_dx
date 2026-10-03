package vue

import (
	"reflect"
	"testing"
)

func wrap(script string) string {
	return "<template>\n  <div>hello</div>\n</template>\n\n<script>\n" + script + "</script>\n\n<style>\n.demo {\n  color: black;\n}\n</style>\n"
}

func TestParseImports(t *testing.T) {
	cases := []struct {
		name   string
		source string
		want   []string
	}{
		{"empty", "", nil},
		{"noScript", "<template><div>x</div></template>\n", nil},
		{"emptyScript", "<script></script>\n", nil},
		{"selfClosingScript", "<script/>\n", nil},
		{"templateOnlyImport", "<template>{{ import \"./fake.vue\" }}</template>\n", nil},
		{"styleOnlyImport", "<style>@import \"./fake.css\";</style>\n", nil},
		{"templateStyleInert", "<template>import \"./fake.vue\"</template>\n<script>\nimport y from './real.vue';\n</script>\n<style>import \"./other.vue\"</style>\n", []string{"real"}},
		{"sideEffect", "import \"./hello.vue\";\n", []string{"hello"}},
		{"sideEffectSingle", "import './helper';\n", []string{"helper"}},
		{"default", "import hello from \"./hello.vue\";\n", []string{"hello"}},
		{"named", "import {a, b} from './util.mjs';\n", []string{"util"}},
		{"namespace", "import * as ns from \"../pkg/demo.vue\";\n", []string{"demo"}},
		{"bare", "import React from \"react\";\n", []string{"react"}},
		{"vueBare", "import { ref } from \"vue\";\n", []string{"vue"}},
		{"scoped", "import x from \"@scope/pkg/sub\";\n", []string{"@scope/pkg/sub"}},
		{"scopedRoot", "import x from \"@scope/pkg\";\n", []string{"@scope/pkg"}},
		{"subpath", "import x from \"pkg/subpath\";\n", []string{"pkg/subpath"}},
		{"nodeBuiltin", "import fs from \"fs\";\n", []string{"fs"}},
		{"nodePrefix", "import fs from \"node:fs\";\n", []string{"node:fs"}},
		{"exportNamed", "export {a} from './helper.vue';\n", []string{"helper"}},
		{"exportStar", "export * from \"./demo.vue\";\n", []string{"demo"}},
		{"exportStarAs", "export * as ns from '../lib.vue';\n", []string{"lib"}},
		{"exportLocal", "export const x = 1;\n", nil},
		{"dynamic", "const m = await import(\"./lazy.vue\");\n", []string{"lazy"}},
		{"dynamicSingle", "import('./other.mjs');\n", []string{"other"}},
		{"dynamicBare", "import(\"react\");\n", []string{"react"}},
		{"require", "const x = require(\"./data.cjs\");\n", []string{"data"}},
		{"requireBare", "require('fs');\n", []string{"fs"}},
		{"requireComputed", "require(name);\n", nil},
		{"dynamicComputed", "import(name);\n", nil},
		{"templateDynamic", "import(`./lazy.vue`);\n", nil},
		{"importMeta", "const u = import.meta.url;\n", nil},
		{"methodRequire", "obj.require(\"./fake.vue\");\n", nil},
		{"noImports", "const x = 1;\n", nil},
		{"lineComment", "// import foo from \"bar\";\n", nil},
		{"blockComment", "/* import \"./hidden.vue\"; */\nconst x = 1;\n", nil},
		{"trailingComment", "import a from \"./real.vue\"; // import \"./fake.vue\";\n", []string{"real"}},
		{"doubleString", "\"import './fake.vue'\";\nimport y from './real.vue';\n", []string{"real"}},
		{"singleString", "'require(\"./fake.vue\")';\n", nil},
		{"templateInert", "`import './fake.vue'`;\n", nil},
		{"regexInert", "const re = /import './fake.vue'/;\nconst x = 1;\n", nil},
		{"unterminated", "\"abc\nimport y from './real.vue';\n", []string{"real"}},
		{"relativeNoExt", "import x from './helper';\n", []string{"helper"}},
		{"relativeDir", "import x from './dir/';\n", []string{"dir"}},
		{"multiline", "import {\n a,\n b\n} from './multi.vue';\n", []string{"multi"}},
		{"exportMultiline", "export {\na\n} from \"./shared.vue\";\n", []string{"shared"}},
		{"duplicate", "import a from './same.vue';\nimport b from './same.vue';\n", []string{"same"}},
		{"emptySpec", "import \"\";\n", nil},
		{"unterminatedBlock", "/* import \"./hidden.vue\";", nil},
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
		{"exportNewlineFrom", "export {a}\nfrom './helper.vue';\n", []string{"helper"}},
		{"exportNewlineNoFrom", "export const x = 1\n", nil},
		{"exportFromUnterminated", "export {a} from \"abc\n", nil},
		{"exportFromNonQuote", "export {a} from bar;\n", nil},
		{"exportEOF", "export", nil},
		{"exportBraceEOF", "export {", nil},
		{"requireNoParen", "require;\n", nil},
		{"requireEOF", "require", nil},
		{"requireMissingParen", "require(\"a\";\n", nil},
		{"requireUnterminated", "require(\"abc\n", nil},
		{"triviaLineComment", "import //c\n\"./a.vue\";\n", []string{"a"}},
		{"triviaBlockComment", "import /*c*/ \"./a.vue\";\n", []string{"a"}},
		{"triviaUnterminatedBlock", "import /* unterminated", nil},
		{"importBraceComment", "import {a /*c*/} from './real.vue';\n", []string{"real"}},
		{"exportBraceComment", "export {a /*c*/} from './real.vue';\n", []string{"real"}},
		{"inertEscape", "\"a\\nb\";\nimport y from './real.vue';\n", []string{"real"}},
		{"templateInterpObject", "`outer ${ {a: 1} } inner`;\nimport y from './real.vue';\n", []string{"real"}},
		{"quotedEscape", "import \"a\\\"b\";\n", []string{"a\"b"}},
		{"quotedBackslashEOF", "import \"abc\\", nil},
		{"quotedEOF", "import \"abc", nil},
		{"templateEscape", "`a\\nb`;\nimport y from './real.vue';\n", []string{"real"}},
		{"templateInterp", "`outer ${x} inner`;\nimport y from './real.vue';\n", []string{"real"}},
		{"templateInterpQuote", "`outer ${'x'} inner`;\nimport y from './real.vue';\n", []string{"real"}},
		{"templateInterpNested", "`outer ${`inner`} end`;\nimport y from './real.vue';\n", []string{"real"}},
		{"templateUnterminated", "`abc", nil},
		{"regexAtStart", "/abc/;\nimport y from './real.vue';\n", []string{"real"}},
		{"division", "const x = a / b;\nimport y from './real.vue';\n", []string{"real"}},
		{"divisionParen", "const x = (a) / b;\nimport y from './real.vue';\n", []string{"real"}},
		{"regexEscape", "const re = /a\\/b/;\nimport y from './real.vue';\n", []string{"real"}},
		{"regexNewline", "const re = /abc\nimport y from './real.vue';\n", []string{"real"}},
		{"regexClass", "const re = /[a/b]/;\nimport y from './real.vue';\n", []string{"real"}},
		{"regexEOF", "const re = /abc", nil},
	}
	for _, tc := range cases {
		var content []byte
		switch tc.name {
		case "empty", "noScript", "emptyScript", "selfClosingScript",
			"templateOnlyImport", "styleOnlyImport", "templateStyleInert":
			content = []byte(tc.source)
		default:
			content = []byte(wrap(tc.source))
		}
		if got := ParseImports(content); !reflect.DeepEqual(got, tc.want) {
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
	for _, name := range []string{"helper", "", "vue", "fs-extra", "react", "fs/promises/extra"} {
		if IsStdLib(name) {
			t.Errorf("non-stdlib identity recognized as stdlib: %q", name)
		}
	}
	if !IsStdLib("fs/promises") {
		t.Error("stdlib subpath fs/promises not recognized")
	}
	if IsStdLib("node:") {
		t.Error("empty node: identity recognized as stdlib")
	}
}
