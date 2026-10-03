package astro

import (
	"reflect"
	"strings"
	"testing"
)

func wrap(script string) string {
	return "---\nconst title = \"hello\";\n---\n\n<script>\n" + script + "</script>\n\n<div class=\"hello\">hello</div>\n\n<style>\n.hello {\n  color: black;\n}\n</style>\n"
}

func TestParseImports(t *testing.T) {
	cases := []struct {
		name   string
		source string
		want   []string
	}{
		{"empty", "", nil},
		{"noRegions", "<div>hello</div>\n", nil},
		{"emptyScript", "---\nconst x = 1;\n---\n<script></script>\n", nil},
		{"selfClosingScript", "<script/>\n", nil},
		{"markupOnlyImport", "<div>{import \"./fake.astro\"}</div>\n", nil},
		{"styleOnlyImport", "<style>@import \"./fake.css\";</style>\n", nil},
		{"markupStyleInert", "---\nconst x = 1;\n---\n<div>import \"./fake.astro\"</div>\n<script>\nimport y from './real.astro';\n</script>\n<style>import \"./other.astro\"</style>\n", []string{"real"}},
		{"frontmatterImport", "---\nimport helper from \"./helper.astro\";\n---\n<div>x</div>\n", []string{"helper"}},
		{"frontmatterAndScript", "---\nimport a from \"./a.astro\";\n---\n<div>x</div>\n<script>\nimport b from \"./b.astro\";\n</script>\n", []string{"a", "b"}},
		{"frontmatterDuplicateScript", "---\nimport a from \"./same.astro\";\n---\n<script>\nimport b from \"./same.astro\";\n</script>\n", []string{"same"}},
		{"frontmatterNoScript", "---\nimport { onMount } from \"astro\";\n---\n", []string{"astro"}},
		{"fenceUnclosedInert", "---\nimport x from \"./ghost.astro\";\n", nil},
		{"fenceUnclosedWithScriptInert", "---\nimport x from \"./ghost.astro\";\n<script>\nimport y from \"./real.astro\";\n</script>\n", nil},
		{"unparseableScriptInert", "---\nimport x from \"./a.astro\";\n---\n<script>const x = 1;", nil},
		{"lateFenceInert", "<div>---</div>\n---\nimport x from \"./ghost.astro\";\n---\n", nil},
		{"fenceIndentedInert", "  ---\nimport x from \"./ghost.astro\";\n---\n", nil},
		{"fenceTrailingSpace", "---  \nimport x from \"./a.astro\";\n---\n", []string{"a"}},
		{"fenceCRLF", "---\r\nimport x from \"./a.astro\";\r\n---\r\n<div>x</div>\n", []string{"a"}},
		{"emptyFence", "---\n---\n<div>x</div>\n", nil},
		{"bareFence", "---", nil},
		{"loneFenceLine", "---\n", nil},
		{"scriptStringInFrontmatter", "---\nconst s = \"<script>\";\nimport x from \"./a.astro\";\n---\n<div>x</div>\n", []string{"a"}},
		{"clientScripts", "---\nconst t = \"demo\";\n---\n<script>\nimport a from './a.astro';\n</script>\n<script>\nimport b from './b.astro';\n</script>\n<div>x</div>\n", []string{"a", "b"}},
		{"sideEffect", "import \"./hello.astro\";\n", []string{"hello"}},
		{"sideEffectSingle", "import './helper';\n", []string{"helper"}},
		{"default", "import hello from \"./hello.astro\";\n", []string{"hello"}},
		{"named", "import {a, b} from './util.mjs';\n", []string{"util"}},
		{"namespace", "import * as ns from \"../pkg/demo.astro\";\n", []string{"demo"}},
		{"bare", "import React from \"react\";\n", []string{"react"}},
		{"astroBare", "import { defineConfig } from \"astro\";\n", []string{"astro"}},
		{"astroCompiler", "import { parse } from \"@astrojs/compiler\";\n", []string{"@astrojs/compiler"}},
		{"scoped", "import x from \"@scope/pkg/sub\";\n", []string{"@scope/pkg/sub"}},
		{"scopedRoot", "import x from \"@scope/pkg\";\n", []string{"@scope/pkg"}},
		{"subpath", "import x from \"pkg/subpath\";\n", []string{"pkg/subpath"}},
		{"nodeBuiltin", "import fs from \"fs\";\n", []string{"fs"}},
		{"nodePrefix", "import fs from \"node:fs\";\n", []string{"node:fs"}},
		{"exportNamed", "export {a} from './helper.astro';\n", []string{"helper"}},
		{"exportStar", "export * from \"./demo.astro\";\n", []string{"demo"}},
		{"exportStarAs", "export * as ns from '../lib.astro';\n", []string{"lib"}},
		{"exportLocal", "export const x = 1;\n", nil},
		{"dynamic", "const m = await import(\"./lazy.astro\");\n", []string{"lazy"}},
		{"dynamicSingle", "import('./other.mjs');\n", []string{"other"}},
		{"dynamicBare", "import(\"react\");\n", []string{"react"}},
		{"require", "const x = require(\"./data.cjs\");\n", []string{"data"}},
		{"requireBare", "require('fs');\n", []string{"fs"}},
		{"requireComputed", "require(name);\n", nil},
		{"dynamicComputed", "import(name);\n", nil},
		{"templateDynamic", "import(`./lazy.astro`);\n", nil},
		{"importMeta", "const u = import.meta.url;\n", nil},
		{"methodRequire", "obj.require(\"./fake.astro\");\n", nil},
		{"noImports", "const x = 1;\n", nil},
		{"lineComment", "// import foo from \"bar\";\n", nil},
		{"blockComment", "/* import \"./hidden.astro\"; */\nconst x = 1;\n", nil},
		{"trailingComment", "import a from \"./real.astro\"; // import \"./fake.astro\";\n", []string{"real"}},
		{"doubleString", "\"import './fake.astro'\";\nimport y from './real.astro';\n", []string{"real"}},
		{"singleString", "'require(\"./fake.astro\")';\n", nil},
		{"templateInert", "`import './fake.astro'`;\n", nil},
		{"regexInert", "const re = /import './fake.astro'/;\nconst x = 1;\n", nil},
		{"unterminated", "\"abc\nimport y from './real.astro';\n", []string{"real"}},
		{"relativeNoExt", "import x from './helper';\n", []string{"helper"}},
		{"relativeDir", "import x from './dir/';\n", []string{"dir"}},
		{"multiline", "import {\n a,\n b\n} from './multi.astro';\n", []string{"multi"}},
		{"exportMultiline", "export {\na\n} from \"./shared.astro\";\n", []string{"shared"}},
		{"duplicate", "import a from './same.astro';\nimport b from './same.astro';\n", []string{"same"}},
		{"emptySpec", "import \"\";\n", nil},
		{"unterminatedBlock", "/* import \"./hidden.astro\";", nil},
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
		{"exportNewlineFrom", "export {a}\nfrom './helper.astro';\n", []string{"helper"}},
		{"exportNewlineNoFrom", "export const x = 1\n", nil},
		{"exportFromUnterminated", "export {a} from \"abc\n", nil},
		{"exportFromNonQuote", "export {a} from bar;\n", nil},
		{"exportEOF", "export", nil},
		{"exportBraceEOF", "export {", nil},
		{"requireNoParen", "require;\n", nil},
		{"requireEOF", "require", nil},
		{"requireMissingParen", "require(\"a\";\n", nil},
		{"requireUnterminated", "require(\"abc\n", nil},
		{"triviaLineComment", "import //c\n\"./a.astro\";\n", []string{"a"}},
		{"triviaBlockComment", "import /*c*/ \"./a.astro\";\n", []string{"a"}},
		{"triviaUnterminatedBlock", "import /* unterminated", nil},
		{"importBraceComment", "import {a /*c*/} from './real.astro';\n", []string{"real"}},
		{"exportBraceComment", "export {a /*c*/} from './real.astro';\n", []string{"real"}},
		{"inertEscape", "\"a\\nb\";\nimport y from './real.astro';\n", []string{"real"}},
		{"templateInterpObject", "`outer ${ {a: 1} } inner`;\nimport y from './real.astro';\n", []string{"real"}},
		{"quotedEscape", "import \"a\\\"b\";\n", []string{"a\"b"}},
		{"quotedBackslashEOF", "import \"abc\\", nil},
		{"quotedEOF", "import \"abc", nil},
		{"templateEscape", "`a\\nb`;\nimport y from './real.astro';\n", []string{"real"}},
		{"templateInterp", "`outer ${x} inner`;\nimport y from './real.astro';\n", []string{"real"}},
		{"templateInterpQuote", "`outer ${'x'} inner`;\nimport y from './real.astro';\n", []string{"real"}},
		{"templateInterpNested", "`outer ${`inner`} end`;\nimport y from './real.astro';\n", []string{"real"}},
		{"templateUnterminated", "`abc", nil},
		{"regexAtStart", "/abc/;\nimport y from './real.astro';\n", []string{"real"}},
		{"division", "const x = a / b;\nimport y from './real.astro';\n", []string{"real"}},
		{"divisionParen", "const x = (a) / b;\nimport y from './real.astro';\n", []string{"real"}},
		{"regexEscape", "const re = /a\\/b/;\nimport y from './real.astro';\n", []string{"real"}},
		{"regexNewline", "const re = /abc\nimport y from './real.astro';\n", []string{"real"}},
		{"regexClass", "const re = /[a/b]/;\nimport y from './real.astro';\n", []string{"real"}},
		{"regexEOF", "const re = /abc", nil},
	}
	raw := map[string]bool{
		"empty": true, "noRegions": true, "emptyScript": true,
		"selfClosingScript": true, "markupOnlyImport": true,
		"styleOnlyImport": true, "markupStyleInert": true,
		"frontmatterImport": true, "frontmatterAndScript": true,
		"frontmatterDuplicateScript": true, "frontmatterNoScript": true,
		"fenceUnclosedInert": true, "fenceUnclosedWithScriptInert": true,
		"unparseableScriptInert": true, "lateFenceInert": true,
		"fenceIndentedInert": true, "fenceTrailingSpace": true,
		"fenceCRLF": true, "emptyFence": true, "bareFence": true,
		"loneFenceLine": true, "scriptStringInFrontmatter": true,
		"clientScripts": true,
	}
	for _, tc := range cases {
		var content []byte
		if raw[tc.name] {
			content = []byte(tc.source)
		} else {
			content = []byte(wrap(tc.source))
		}
		if got := ParseImports(content); !reflect.DeepEqual(got, tc.want) {
			t.Errorf("%s: ParseImports = %q, want %q", tc.name, got, tc.want)
		}
	}
}

func TestSplitFence(t *testing.T) {
	cases := []struct {
		name       string
		source     string
		wantFenced bool
		wantFront  string
		wantRest   string
		wantOK     bool
	}{
		{"empty", "", false, "", "", false},
		{"noFence", "<div>x</div>\n", false, "", "", false},
		{"basic", "---\nconst x = 1;\n---\n<div/>\n", true, "const x = 1;\n", "<div/>\n", true},
		{"noTrailingNewline", "---\nconst x = 1;\n---", true, "const x = 1;\n", "", true},
		{"emptyFront", "---\n---\n", true, "", "", true},
		{"unclosed", "---\nconst x = 1;\n", true, "", "", false},
		{"bareFence", "---", true, "", "", false},
		{"loneFenceLine", "---\n", true, "", "", false},
		{"lateFence", "<div/>\n---\n---\n", false, "", "", false},
		{"indented", "  ---\n---\n", false, "", "", false},
		{"trailingSpace", "--- \t \nconst x = 1;\n---\n", true, "const x = 1;\n", "", true},
		{"crlf", "---\r\nconst x = 1;\r\n---\r\n", true, "const x = 1;\r\n", "", true},
	}
	for _, tc := range cases {
		fenced, front, rest, ok := splitFence([]byte(tc.source))
		if fenced != tc.wantFenced || string(front) != tc.wantFront || string(rest) != tc.wantRest || ok != tc.wantOK {
			t.Errorf("%s: splitFence = (%v, %q, %q, %v), want (%v, %q, %q, %v)",
				tc.name, fenced, front, rest, ok, tc.wantFenced, tc.wantFront, tc.wantRest, tc.wantOK)
		}
	}
}

func TestExtractScripts(t *testing.T) {
	cases := []struct {
		name    string
		source  string
		want    []string
		wantNil bool
	}{
		{"basic", "<script>\nconst x = 1;\n</script>", []string{"const x = 1;"}, false},
		{"upper", "<SCRIPT>\nconst x = 1;\n</SCRIPT>", []string{"const x = 1;"}, false},
		{"quotedAttr", "<script context=\"a>b\">\nconst x = 1;\n</script>", []string{"const x = 1;"}, false},
		{"singleQuotedAttr", "<script context='a>b'>\nconst x = 1;\n</script>", []string{"const x = 1;"}, false},
		{"afterMarkup", "<div>t</div><custom-element foo=\"bar\"/><my_tag/><x:y/></template><script>const x = 1;</script>", []string{"const x = 1;"}, false},
		{"commentedScript", "<!-- <script>import './fake.astro';</script> --><script>const x = 1;</script>", []string{"const x = 1;"}, false},
		{"commentInScript", "<script>const x = 1;<!-- not html -->const y = 2;</script>", []string{"const x = 1;<!-- not html -->const y = 2;"}, false},
		{"selfClosingSkipped", "<script/>\n<script>const x = 1;</script>", []string{"const x = 1;"}, false},
		{"closingFirst", "</script><script>const x = 1;</script>", []string{"const x = 1;"}, false},
		{"noName", "<>text</><script>const x = 1;</script>", []string{"const x = 1;"}, false},
		{"scriptPrefix", "<scriptx>no</scriptx><script>const x = 1;</script>", []string{"const x = 1;"}, false},
		{"none", "<div>x</div>", nil, true},
		{"selfClosingOnly", "<script/>", nil, true},
		{"unclosedTag", "<script", nil, true},
		{"unclosedAttr", "<script lang=\"ts\"", nil, true},
		{"unterminatedAttrQuote", "<script lang=\"ts>", nil, true},
		{"noClose", "<script>const x = 1;", nil, true},
		{"partialSecondInert", "<script>const a = 1;</script><script>const b = 2;", nil, true},
		{"unterminatedComment", "<!-- <script>", nil, true},
		{"unterminatedCommentInScript", "<script>const x = 1;<!--", nil, true},
	}
	for _, tc := range cases {
		got := ExtractScripts([]byte(tc.source))
		if tc.wantNil {
			if got != nil {
				t.Errorf("%s: ExtractScripts = %q, want nil", tc.name, got)
			}
			continue
		}
		if len(got) != len(tc.want) {
			t.Errorf("%s: ExtractScripts = %q, want %q", tc.name, got, tc.want)
			continue
		}
		for i := range got {
			if strings.TrimSpace(string(got[i])) != tc.want[i] {
				t.Errorf("%s: ExtractScripts[%d] = %q, want %q", tc.name, i, got[i], tc.want[i])
			}
		}
	}
}

func TestOpensScript(t *testing.T) {
	for _, source := range []string{
		"<script>const x = 1;</script>",
		"<SCRIPT>",
		"<script",
		"<div>x</div><script src=\"a.js\">",
		"<!-- <script> --><script>",
	} {
		if !opensScript([]byte(source)) {
			t.Errorf("opensScript(%q) = false, want true", source)
		}
	}
	for _, source := range []string{
		"",
		"<div>x</div>",
		"<scriptx>no</scriptx>",
		"<!-- <script> -->",
		"<!-- <script>",
		"</script>",
		"a < b",
		"trailing <",
	} {
		if opensScript([]byte(source)) {
			t.Errorf("opensScript(%q) = true, want false", source)
		}
	}
}

func TestIsStdLib(t *testing.T) {
	for _, name := range []string{"fs", "path", "node:fs", "node:path", "test", "node:test"} {
		if !IsStdLib(name) {
			t.Errorf("stdlib identity not recognized: %q", name)
		}
	}
	for _, name := range []string{"helper", "", "astro", "fs-extra", "react", "fs/promises/extra"} {
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

func TestNormalizeSpec(t *testing.T) {
	cases := []struct{ in, want string }{
		{"./hello.astro", "hello"},
		{"../pkg/demo.mjsx", "demo"},
		{"./helper", "helper"},
		{"./dir/", "dir"},
		{"/abs/path.astro", "path"},
		{"astro", "astro"},
		{"@astrojs/compiler", "@astrojs/compiler"},
		{"@scope/pkg/sub", "@scope/pkg/sub"},
		{"node:fs", "node:fs"},
		{"", ""},
		{"./", ""},
		{"   ", ""},
	}
	for _, tc := range cases {
		if got := normalizeSpec(tc.in); got != tc.want {
			t.Errorf("normalizeSpec(%q) = %q, want %q", tc.in, got, tc.want)
		}
	}
}
