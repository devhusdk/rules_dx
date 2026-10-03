package common

import (
	"reflect"
	"testing"
)

func TestNormalize(t *testing.T) {
	cases := []struct{ in, want string }{
		{"hello", "hello"},
		{"my-mod", "my_mod"},
		{"UPPER123", "UPPER123"},
	}
	for _, tc := range cases {
		if got, err := Normalize(tc.in); err != nil || got != tc.want {
			t.Errorf("Normalize(%q) = %q, %v; want %q", tc.in, got, err, tc.want)
		}
	}
	if _, err := Normalize("---"); err == nil {
		t.Errorf("Normalize(%q) succeeded, want error", "---")
	}
}

func TestCheckCollisions(t *testing.T) {
	if err := CheckCollisions([]Claimant{{Name: "a", Source: "a.js", Kind: "k"}}); err != nil {
		t.Fatalf("CheckCollisions = %v, want nil", err)
	}
	err := CheckCollisions([]Claimant{
		{Name: "a", Source: "a.js", Kind: "k"},
		{Name: "a", Source: "b.js", Kind: "k"},
	})
	if err == nil {
		t.Fatalf("CheckCollisions succeeded, want collision")
	}
}

func collectScan(src []byte) []string {
	var out []string
	Scan(src, func(spec string) {
		out = append(out, NormalizeJSSpec(spec))
	})
	return out
}

func collectEmbedded(src []byte) []string {
	var out []string
	ScanEmbedded(src, func(spec string) {
		out = append(out, NormalizeJSSpec(spec))
	})
	return out
}

func TestScanPlain(t *testing.T) {
	got := collectScan([]byte("import hello from \"./hello.js\";\n"))
	if !reflect.DeepEqual(got, []string{"hello"}) {
		t.Fatalf("Scan = %q, want [hello]", got)
	}
}

func TestScanEmbedded(t *testing.T) {
	src := []byte("<template><p>x</p></template><script>import y from \"./why.js\";</script>")
	got := collectEmbedded(ExtractScript(src))
	if !reflect.DeepEqual(got, []string{"why"}) {
		t.Fatalf("ScanEmbedded = %q, want [why]", got)
	}
}

func TestScanTemplateSubstitution(t *testing.T) {
	cases := []struct{ name, src string }{
		{"open brace in string", "const s = `a${ \"${\" }b`;\nimport hidden from \"./hidden.js\";\n"},
		{"close brace then backtick in string", "const s = `a${ \"x}\" + `inner` }`;\nimport hidden from \"./hidden.js\";\n"},
		{"nested template in substitution", "const s = `a${ b(`c`) }d`;\nimport hidden from \"./hidden.js\";\n"},
		{"quoted line comment", "const s = `a${ \"//\" }b`;\nimport hidden from \"./hidden.js\";\n"},
	}
	for _, tc := range cases {
		if got := collectScan([]byte(tc.src)); !reflect.DeepEqual(got, []string{"hidden"}) {
			t.Errorf("Scan %s = %q, want [hidden]", tc.name, got)
		}
		if got := collectEmbedded([]byte(tc.src)); !reflect.DeepEqual(got, []string{"hidden"}) {
			t.Errorf("ScanEmbedded %s = %q, want [hidden]", tc.name, got)
		}
	}
}

func TestScanEmbeddedTricks(t *testing.T) {
	cases := []struct {
		name, src string
		want      []string
	}{
		{"block comment", "/* hello */ import a from \"./a.js\";", []string{"a"}},
		{"unterminated block comment", "/* unterminated", nil},
		{"trailing block comment", "import a from \"./a.js\"; /* trailing */", []string{"a"}},
		{"regex prefix", "/abc/; import a from \"./a.js\";", []string{"a"}},
		{"division", "const x = a / b; import y from \"./real.js\";", []string{"real"}},
		{"division paren", "const x = (a) / b; import y from \"./real.js\";", []string{"real"}},
		{"regex escape", "const re = /a\\/b/; import y from \"./real.js\";", []string{"real"}},
		{"regex class", "const re = /[a/b]/; import y from \"./real.js\";", []string{"real"}},
		{"regex flags", "const re = /abc/gi; import y from \"./real.js\";", []string{"real"}},
		{"regex newline", "const re = /abc\nimport y from \"./real.js\";", []string{"real"}},
		{"regex EOF", "const re = /abc", nil},
		{"require", "require(\"./a.js\");", []string{"a"}},
		{"method require", "obj.require(\"./fake.js\");", nil},
		{"spaced dot require", "obj . require(\"./fake.js\");", nil},
		{"require no paren", "require;", nil},
		{"require EOF", "require", nil},
		{"require missing paren", "require(\"a\";", nil},
		{"require unterminated", "require(\"abc\n\");", nil},
		{"import and require", "import a from \"./a.js\"; require(\"./b.js\");", []string{"a", "b"}},
		{"quoted escape", "\"a\\\"b\"; import a from \"./a.js\";", []string{"a"}},
		{"template escape", "`a\\nb`; import a from \"./a.js\";", []string{"a"}},
		{"template object", "`outer ${ {a: 1} } inner`; import y from \"./real.js\";", []string{"real"}},
		{"template quote", "`outer ${'x'} inner`; import y from \"./real.js\";", []string{"real"}},
		{"template nested", "`outer ${`inner`} end`; import y from \"./real.js\";", []string{"real"}},
		{"template double quote", "`outer ${\"x\"} inner`; import y from \"./real.js\";", []string{"real"}},
	}
	for _, tc := range cases {
		if got := collectEmbedded([]byte(tc.src)); !reflect.DeepEqual(got, tc.want) {
			t.Errorf("ScanEmbedded %s = %q, want %q", tc.name, got, tc.want)
		}
	}
}

func TestSkipQuotedAndTemplateEOF(t *testing.T) {
	if got := skipQuoted([]byte("\"abc"), 0); got != 4 {
		t.Errorf("skipQuoted EOF = %d, want 4", got)
	}
	if got := skipTemplate([]byte("`abc"), 0); got != 4 {
		t.Errorf("skipTemplate EOF = %d, want 4", got)
	}
}

func TestIsPrecededByDot(t *testing.T) {
	cases := []struct {
		src  string
		pos  int
		want bool
	}{
		{"require", 0, false},
		{"obj.require", 4, true},
		{"obj . \n require", 8, true},
		{"x require", 2, false},
	}
	for _, tc := range cases {
		if got := isPrecededByDot([]byte(tc.src), tc.pos); got != tc.want {
			t.Errorf("isPrecededByDot(%q, %d) = %v, want %v", tc.src, tc.pos, got, tc.want)
		}
	}
}

func TestIsRegexStart(t *testing.T) {
	cases := []struct {
		src  string
		pos  int
		want bool
	}{
		{"/abc", 0, true},
		{"a/b", 1, false},
		{"(a)/b", 3, false},
		{"[a]/b", 3, false},
		{"{a}/b", 3, false},
		{"\"a\"/b", 3, false},
		{";/b", 1, true},
		{"   /abc", 3, true},
		{"f()/b", 3, false},
		{"}/b", 1, false},
		{"`a`/b", 3, false},
	}
	for _, tc := range cases {
		if got := isRegexStart([]byte(tc.src), tc.pos); got != tc.want {
			t.Errorf("isRegexStart(%q, %d) = %v, want %v", tc.src, tc.pos, got, tc.want)
		}
	}
}

func TestNormalizeJSSpec(t *testing.T) {
	cases := []struct{ in, want string }{
		{"", ""},
		{"   ", ""},
		{"./", ""},
		{".", ""},
		{"/", ""},
		{"./dir/", "dir"},
		{"./helper", "helper"},
		{"./.hidden", ".hidden"},
		{"../pkg/demo.mjsx", "demo"},
		{"/abs/path.mdx", "path"},
		{"react", "react"},
		{"@mdx-js/mdx", "@mdx-js/mdx"},
		{"@scope/pkg/sub", "@scope/pkg/sub"},
		{"node:fs", "node:fs"},
	}
	for _, ext := range []string{
		".js", ".jsx", ".mjs", ".cjs",
		".ts", ".tsx", ".mts", ".cts",
		".mdx", ".vue", ".svelte", ".astro",
	} {
		cases = append(cases, struct{ in, want string }{"./hello" + ext, "hello"})
	}
	for _, tc := range cases {
		if got := NormalizeJSSpec(tc.in); got != tc.want {
			t.Errorf("NormalizeJSSpec(%q) = %q, want %q", tc.in, got, tc.want)
		}
	}
}

func TestIsNodeBuiltin(t *testing.T) {
	for _, name := range []string{"fs", "path", "node:fs", "node:path", "test", "node:test"} {
		if !IsNodeBuiltin(name) {
			t.Errorf("node builtin not recognized: %q", name)
		}
	}
	if !IsNodeBuiltin("fs/promises") {
		t.Error("node builtin subpath fs/promises not recognized")
	}
	for _, name := range []string{
		"", "node:", "node:node:fs", "helper", "fs-extra", "react",
		"fs/promises/extra", "@mdx-js/mdx", "astro", "svelte", "vue",
	} {
		if IsNodeBuiltin(name) {
			t.Errorf("non-builtin identity recognized as node builtin: %q", name)
		}
	}
}

func TestParseImportRefs(t *testing.T) {
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
		{"manyRoots", "import z from './z.js';\nimport u from './u.js';\nimport t from './t.js';\nimport s from './s.js';\nimport r from 'react';\nimport q from './q.js';\nimport p from './p.js';\n", []string{"p", "q", "react", "s", "t", "u", "z"}},
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
		{"exportDecl", "export declare const x: number;\n", nil},
		{"exportInterface", "export interface A { x: number }\n", nil},
	}
	for _, tc := range cases {
		var got []string
		for _, ref := range ParseImportRefs([]byte(tc.source)) {
			got = append(got, ref.Root)
		}
		if !reflect.DeepEqual(got, tc.want) {
			t.Errorf("%s: ParseImportRefs = %q, want %q", tc.name, got, tc.want)
		}
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
		if !isRelativeSpec(spec) {
			t.Errorf("isRelativeSpec(%q) = false, want true", spec)
		}
	}
	for _, spec := range []string{"react", "fs", "node:fs", ""} {
		if isRelativeSpec(spec) {
			t.Errorf("isRelativeSpec(%q) = true, want false", spec)
		}
	}
}

func TestSpecSetRoots(t *testing.T) {
	var set SpecSet
	if got := set.Roots(); got != nil {
		t.Errorf("empty Roots = %q, want nil", got)
	}
	set.Add("")
	set.Add("./")
	if got := set.Roots(); got != nil {
		t.Errorf("unnormalizable Roots = %q, want nil", got)
	}
	set.Add("./b.js")
	set.Add("./a.mjs")
	set.Add("./a.tsx")
	set.Add("react")
	set.Add(" ./a.js ")
	if got := set.Roots(); !reflect.DeepEqual(got, []string{"a", "b", "react"}) {
		t.Errorf("Roots = %q, want [a b react]", got)
	}
}

func TestExtractScripts(t *testing.T) {
	one := func(s string) []byte { return []byte(s) }
	cases := []struct {
		name   string
		source string
		first  []byte
		all    [][]byte
	}{
		{"empty", "", nil, nil},
		{"none", "<div>x</div>", nil, nil},
		{"noName", "<>text</><script>\nconst x = 1;\n</script>", one("\nconst x = 1;\n"), [][]byte{one("\nconst x = 1;\n")}},
		{"basic", "<script>\nconst x = 1;\n</script>", one("\nconst x = 1;\n"), [][]byte{one("\nconst x = 1;\n")}},
		{"emptyBody", "<script></script>", []byte{}, [][]byte{{}}},
		{"setup", "<script setup>\nimport x from './a.vue';\n</script>", one("\nimport x from './a.vue';\n"), [][]byte{one("\nimport x from './a.vue';\n")}},
		{"setupLang", "<script setup lang=\"ts\">\nconst x = 1;\n</script>", one("\nconst x = 1;\n"), [][]byte{one("\nconst x = 1;\n")}},
		{"module", "<script context=\"module\">\nconst x = 1;\n</script>", one("\nconst x = 1;\n"), [][]byte{one("\nconst x = 1;\n")}},
		{"genericsAttr", "<script lang=\"ts\" generics=\"T\">\nconst x = 1;\n</script>", one("\nconst x = 1;\n"), [][]byte{one("\nconst x = 1;\n")}},
		{"both", "<script context=\"module\">\nconst a = 1;\n</script><script>\nconst b = 2;\n</script>", one("\nconst a = 1;\n"), [][]byte{one("\nconst a = 1;\n"), one("\nconst b = 2;\n")}},
		{"upper", "<SCRIPT>\nconst x = 1;\n</SCRIPT>", one("\nconst x = 1;\n"), [][]byte{one("\nconst x = 1;\n")}},
		{"closingFirst", "</script><script>\nconst x = 1;\n</script>", one("\nconst x = 1;\n"), [][]byte{one("\nconst x = 1;\n")}},
		{"quotedAttr", "<script lang=\"a>b\">\nconst x = 1;\n</script>", one("\nconst x = 1;\n"), [][]byte{one("\nconst x = 1;\n")}},
		{"singleQuotedAttr", "<script lang='a>b'>\nconst x = 1;\n</script>", one("\nconst x = 1;\n"), [][]byte{one("\nconst x = 1;\n")}},
		{"scriptPrefix", "<scriptx>no</scriptx><script>\nconst x = 1;\n</script>", one("\nconst x = 1;\n"), [][]byte{one("\nconst x = 1;\n")}},
		{"afterMarkup", "<div>t</div><custom-element foo=\"bar\"/><my_tag/><x:y/></template><script>\nconst x = 1;\n</script><script>\nconst y = 2;\n</script>", one("\nconst x = 1;\n"), [][]byte{one("\nconst x = 1;\n"), one("\nconst y = 2;\n")}},
		{"commentedScript", "<!-- <script>import './fake.svelte';</script> --><script>\nconst x = 1;\n</script>", one("\nconst x = 1;\n"), [][]byte{one("\nconst x = 1;\n")}},
		{"commentInScript", "<script>const x = 1;<!-- not html -->const y = 2;</script>", one("const x = 1;<!-- not html -->const y = 2;"), [][]byte{one("const x = 1;<!-- not html -->const y = 2;")}},
		{"selfClosingOnly", "<script/>", nil, nil},
		{"selfClosingFirst", "<script/>\n<script>\nconst x = 1;\n</script>", one("\nconst x = 1;\n"), [][]byte{one("\nconst x = 1;\n")}},
		{"partialSecondKillsAll", "<script>\nconst a = 1;\n</script><script>\nconst b = 2;", nil, nil},
		{"unclosedTag", "<script", nil, nil},
		{"unclosedAttr", "<script lang=\"ts\"", nil, nil},
		{"unterminatedAttrQuote", "<script lang=\"ts>", nil, nil},
		{"noClose", "<script>\nconst x = 1;", nil, nil},
		{"unterminatedComment", "<!-- <script>", nil, nil},
		{"unterminatedCommentInScript", "<script>\nconst x = 1;<!--", nil, nil},
	}
	for _, tc := range cases {
		if got := ExtractScript([]byte(tc.source)); !reflect.DeepEqual(got, tc.first) {
			t.Errorf("ExtractScript %s = %q, want %q", tc.name, got, tc.first)
		}
		if got := ExtractScripts([]byte(tc.source)); !reflect.DeepEqual(got, tc.all) {
			t.Errorf("ExtractScripts %s = %q, want %q", tc.name, got, tc.all)
		}
	}
}

func TestOpensTag(t *testing.T) {
	cases := []struct {
		name, source, tag string
		want              bool
	}{
		{"basic", "<script>const x = 1;</script>", "script", true},
		{"upper", "<SCRIPT>", "script", true},
		{"noClose", "<script", "script", true},
		{"afterMarkup", "<div>x</div><script src=\"a.js\">", "script", true},
		{"afterComment", "<!-- <script> --><script>", "script", true},
		{"commentedOut", "<!-- <script> -->", "script", false},
		{"unterminatedComment", "<!-- <script>", "script", false},
		{"empty", "", "script", false},
		{"markupOnly", "<div>x</div>", "script", false},
		{"namePrefix", "<scriptx>no</scriptx>", "script", false},
		{"closingOnly", "</script>", "script", false},
		{"bareLessThan", "a < b", "script", false},
		{"trailingLessThan", "trailing <", "script", false},
		{"style", "<style>a{}</style>", "style", true},
		{"otherName", "<div>x</div>", "script", false},
	}
	for _, tc := range cases {
		if got := OpensTag([]byte(tc.source), tc.tag); got != tc.want {
			t.Errorf("OpensTag(%q, %q) = %v, want %v", tc.source, tc.tag, got, tc.want)
		}
	}
}

func TestMaskStyles(t *testing.T) {
	cases := []struct{ name, in, want string }{
		{"line comment", "import a.B;\n// import hidden.C;\n", "import a.B;\n                   \n"},
		{"block comment", "import a.B;\n/* import hidden.C; */\n", "import a.B;\n                      \n"},
		{"nested open is not nested", "/* /* a */ b */\n", "           b */\n"},
		{"string", "x = \"import a.B;\";\n", "x =              ;\n"},
		{"char literal", "c = '\"';\n", "c =    ;\n"},
		{"text block", "x = \"\"\"\nimport a.B;\n\"\"\";\n", "x =    \n           \n   ;\n"},
		{"unterminated block", "import a.B;\n/* import hidden.C;\n", "import a.B;\n                   \n"},
		{"unterminated string", "import a.B;\n\"import hidden.C;\n", "import a.B;\n                 \n"},
		{"annotation is not verbatim", "@Foo(\"x\")\n", "@Foo(   )\n"},
	}
	for _, tc := range cases {
		for name, mask := range map[string]func([]byte) []byte{
			"java": MaskJavaStyle, "csharp": MaskCSharpStyle,
		} {
			if got := string(mask([]byte(tc.in))); got != tc.want {
				t.Errorf("%s %s = %q, want %q", name, tc.name, got, tc.want)
			}
		}
	}
}

func TestMaskMultiLineLiterals(t *testing.T) {
	cases := []struct{ name, in, want string }{
		{"verbatim", "x = @\"\nusing a.B;\n\";\n", "x =   \n          \n ;\n"},
		{"interpolated verbatim", "x = $@\"\nusing a.B;\n\";\n", "x = $  \n          \n ;\n"},
		{"verbatim escape", "x = @\"say \"\"hi\"\"\";\nusing a.B;\n", "x =              ;\nusing a.B;\n"},
		{"raw", "x = \"\"\"\nusing a.B;\n\"\"\";\n", "x =    \n          \n   ;\n"},
		{"unterminated verbatim", "x = @\"\nusing a.B;\n", "x =   \n          \n"},
		{"unterminated raw", "x = \"\"\"\nusing a.B;\n", "x =    \n          \n"},
	}
	for _, tc := range cases {
		if got := string(MaskCSharpStyle([]byte(tc.in))); got != tc.want {
			t.Errorf("csharp %s = %q, want %q", tc.name, got, tc.want)
		}
	}
}

func TestMaskFSharpStyle(t *testing.T) {
	cases := []struct{ name, in, want string }{
		{"block comment", "open a.B\n(* open hidden.C *)\n", "open a.B\n                   \n"},
		{"nested block comment", "(* a (* open hidden.C *) b *) open d.E\n", "                              open d.E\n"},
		{"unterminated block comment", "(* open hidden.C\n", "                \n"},
		{"verbatim", "let s = \"\"\"\nopen hidden.C\n\"\"\"\n", "let s =    \n             \n   \n"},
		{"line comment", "open a.B\n// open hidden.C\n", "open a.B\n                \n"},
	}
	for _, tc := range cases {
		if got := string(MaskFSharpStyle([]byte(tc.in))); got != tc.want {
			t.Errorf("%s = %q, want %q", tc.name, got, tc.want)
		}
	}
}

func TestNormalizeDotted(t *testing.T) {
	std := func(name string) bool { return name == "java.util.List" }
	cases := []struct {
		in       string
		onDemand bool
		want     string
	}{
		{"a.b.C", false, "C"},
		{"  a.b.C  ", false, "C"},
		{"C", false, "C"},
		{"", false, ""},
		{"   ", false, ""},
		{"java.util.List", false, "java.util.List"},
		{"a.b.c", true, "c"},
		{"a.b", true, "b"},
	}
	for _, tc := range cases {
		if got := NormalizeDotted(tc.in, tc.onDemand, std); got != tc.want {
			t.Errorf("NormalizeDotted(%q, %v) = %q, want %q", tc.in, tc.onDemand, got, tc.want)
		}
	}
}
