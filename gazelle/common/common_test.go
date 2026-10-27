package common

import (
	"bytes"
	"reflect"
	"sort"
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

func normalizeJSSpec(spec string) string {
	var set SpecSet
	set.Add(spec)
	if roots := set.Roots(); len(roots) > 0 {
		return roots[0]
	}
	return ""
}

func collectScan(src []byte) []string {
	var out []string
	Scan(src, func(spec string) {
		out = append(out, normalizeJSSpec(spec))
	})
	return out
}

func collectEmbedded(src []byte) []string {
	var out []string
	ScanEmbedded(src, func(spec string) {
		out = append(out, normalizeJSSpec(spec))
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
	src := []byte("<template><p>x</p></template><script>import y from \"./why.js\";</script><script setup>import z from \"./zed.js\";</script>")
	var got []string
	for _, script := range ExtractScripts(src, TopLevel) {
		got = append(got, collectEmbedded(script)...)
	}
	if !reflect.DeepEqual(got, []string{"why", "zed"}) {
		t.Fatalf("ScanEmbedded = %q, want [why zed]", got)
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

func TestScanEmbeddedNoSemicolons(t *testing.T) {
	prefixes := []string{"export let x", "export let x = \"\"", "const b = import.meta.env.BASE_URL", "import.meta.hot"}
	followers := []struct{ src, want string }{
		{"import './side.js'", "side"},
		{"import s from './static.js'", "static"},
		{"const r = require('./r.js')", "r"},
	}
	for _, prefix := range prefixes {
		for _, f := range followers {
			for _, semi := range []string{"", ";"} {
				src := prefix + semi + "\n" + f.src + semi + "\n"
				if got := collectEmbedded([]byte(src)); !reflect.DeepEqual(got, []string{f.want}) {
					t.Errorf("ScanEmbedded %q = %q, want [%s]", src, got, f.want)
				}
			}
		}
	}
	cases := []struct {
		src  string
		want []string
	}{
		{"export const r = require('./r.js')", []string{"r"}},
		{"export default { c: () => import('./lazy.js') }", []string{"lazy"}},
		{"export { a }\nimport './side.js'", []string{"side"}},
		{"export * as ns\nfrom './x.js'", []string{"x"}},
		{"export * as ns\nimport './side.js'", []string{"side"}},
		{"export type { T } from './t.js'", []string{"t"}},
	}
	for _, tc := range cases {
		if got := collectEmbedded([]byte(tc.src)); !reflect.DeepEqual(got, tc.want) {
			t.Errorf("ScanEmbedded %q = %q, want %q", tc.src, got, tc.want)
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

func TestSpecSetIdentity(t *testing.T) {
	cases := []struct{ in, want string }{
		{"", ""},
		{"   ", ""},
		{"./", ""},
		{".", ""},
		{"/", ""},
		{"./dir/", "dir"},
		{"./helper", "helper"},
		{"./.hidden", ".hidden"},
		{"../pkg/demo.mjsx", "../pkg/demo.mjsx"},
		{"/abs/path.mdx", "abs/path"},
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
		if got := normalizeJSSpec(tc.in); got != tc.want {
			t.Errorf("normalizeJSSpec(%q) = %q, want %q", tc.in, got, tc.want)
		}
	}
}

func TestSpecSetKeepsDirectories(t *testing.T) {
	set := SpecSet{Dir: "app/web"}
	set.Add("../a/util")
	set.Add("../b/util.ts")
	set.Add("./util")
	set.Add("react")
	want := []string{"app/b/util", "app/web/util", "app/a/util", "react"}
	sort.Strings(want)
	if got := set.Roots(); !reflect.DeepEqual(got, want) {
		t.Errorf("Roots = %q, want %q", got, want)
	}
}

func TestIsNodeBuiltin(t *testing.T) {
	for _, name := range []string{"fs", "path", "node:fs", "node:path", "node:test", "node:sqlite", "node:sea", "node:ffi", "node:test/reporters"} {
		if !IsNodeBuiltin(name) {
			t.Errorf("node builtin not recognized: %q", name)
		}
	}
	if !IsNodeBuiltin("fs/promises") {
		t.Error("node builtin subpath fs/promises not recognized")
	}
	for _, name := range []string{
		"", "node:", "node:node:fs", "test", "sqlite", "sea", "ffi", "test/reporters", "helper", "fs-extra", "react",
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
		{"commentFromInImport", "import {\n a, // from utils\n b,\n} from './x';", []string{"x"}},
		{"commentSemiInImport", "import { a /* ; */, b } from './x';", []string{"x"}},
		{"commentFromInExport", "export {\n a, // from y\n} from './x';", []string{"x"}},
		{"commentBraceInExport", "export { a /* } */ } from './x';", []string{"x"}},
		{"exportLineComment", "export const x = 1 // c\nimport y from './z';", []string{"z"}},
		{"unclosedCommentInImport", "import { a /* b", nil},
		{"importEquals", "import x = require('./z');", []string{"z"}},
		{"importEqualsType", "import type x = require('./z');", []string{"z"}},
		{"importTypeEquals", "import type = require('./z');", []string{"z"}},
		{"exportImportEquals", "export import x = require('./z');", []string{"z"}},
		{"importEqualsDynamic", "import x = require(name);", nil},
		{"importEqualsAlias", "import A = B.C;\nimport y from './real.js';", []string{"real"}},
		{"importEqualsNumber", "import x = 1;", nil},
		{"importEqualsEOF", "import x =", nil},
		{"sideEffect", "import \"./hello.js\";\n", []string{"hello"}},
		{"sideEffectSingle", "import './helper';\n", []string{"helper"}},
		{"default", "import hello from \"./hello.js\";\n", []string{"hello"}},
		{"named", "import {a, b} from './util.mjs';\n", []string{"util"}},
		{"namespace", "import * as ns from \"../pkg/demo.jsx\";\n", []string{"../pkg/demo"}},
		{"bare", "import React from \"react\";\n", []string{"react"}},
		{"scoped", "import x from \"@scope/pkg/sub\";\n", []string{"@scope/pkg/sub"}},
		{"scopedRoot", "import x from \"@scope/pkg\";\n", []string{"@scope/pkg"}},
		{"subpath", "import x from \"pkg/subpath\";\n", []string{"pkg/subpath"}},
		{"nodeBuiltin", "import fs from \"fs\";\n", []string{"fs"}},
		{"nodePrefix", "import fs from \"node:fs\";\n", []string{"node:fs"}},
		{"exportNamed", "export {a} from './helper.js';\n", []string{"helper"}},
		{"exportStar", "export * from \"./demo.cjs\";\n", []string{"demo"}},
		{"exportStarAs", "export * as ns from '../lib.js';\n", []string{"../lib"}},
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
		for _, ref := range ParseImportRefs([]byte(tc.source), "") {
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
		{"relativeStdlibCollision", "import { fmt } from \"./util.js\";\n", []ImportRef{{Spec: "./util.js", Root: "pkg/util", Relative: true}}},
		{"bareStdlib", "import fs from \"fs\";\n", []ImportRef{{Spec: "fs", Root: "fs"}}},
		{"bareNonStdlib", "import React from \"react\";\n", []ImportRef{{Spec: "react", Root: "react"}}},
		{"mixedCollision", "import { fmt } from \"./util.js\";\nimport u from \"util\";\n", []ImportRef{{Spec: "./util.js", Root: "pkg/util", Relative: true}, {Spec: "util", Root: "util"}}},
		{"absolute", "import x from \"/abs/path.js\";\n", []ImportRef{{Spec: "/abs/path.js", Root: "abs/path", Relative: true}}},
		{"repeatedBasenames", "import a from \"../a/util\";\nimport b from \"../b/util.ts\";\n", []ImportRef{{Spec: "../a/util", Root: "a/util", Relative: true}, {Spec: "../b/util.ts", Root: "b/util", Relative: true}}},
		{"index", "import c from \"./components\";\n", []ImportRef{{Spec: "./components", Root: "pkg/components", Relative: true}}},
		{"dotted", "import c from \"./config.prod\";\n", []ImportRef{{Spec: "./config.prod", Root: "pkg/config.prod", Ext: ".prod", Relative: true}}},
		{"json", "import d from \"./data.json\";\n", []ImportRef{{Spec: "./data.json", Root: "pkg/data.json", Ext: ".json", Relative: true}}},
		{"css module", "import s from \"./styles.module.css\";\n", []ImportRef{{Spec: "./styles.module.css", Root: "pkg/styles.module.css", Ext: ".css", Relative: true}}},
		{"mts", "import x from \"./x.mts\";\n", []ImportRef{{Spec: "./x.mts", Root: "pkg/x", Relative: true}}},
		{"importEqualsRelative", "import x = require(\"./z.js\");\n", []ImportRef{{Spec: "./z.js", Root: "pkg/z", Relative: true}}},
		{"commentInList", "import {\n a, // from utils\n} from './x';\n", []ImportRef{{Spec: "./x", Root: "pkg/x", Relative: true}}},
		{"sameTarget", "import a from \"./x/../util.js\";\nimport b from \"./util\";\n", []ImportRef{{Spec: "./x/../util.js", Root: "pkg/util", Relative: true}}},
	}
	for _, tc := range cases {
		if got := ParseImportRefs([]byte(tc.source), "pkg"); !reflect.DeepEqual(got, tc.want) {
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
		all    [][]byte
	}{
		{"empty", "", nil},
		{"none", "<div>x</div>", nil},
		{"noName", "<>text</><script>\nconst x = 1;\n</script>", [][]byte{one("\nconst x = 1;\n")}},
		{"basic", "<script>\nconst x = 1;\n</script>", [][]byte{one("\nconst x = 1;\n")}},
		{"emptyBody", "<script></script>", [][]byte{{}}},
		{"setup", "<script setup>\nimport x from './a.vue';\n</script>", [][]byte{one("\nimport x from './a.vue';\n")}},
		{"setupLang", "<script setup lang=\"ts\">\nconst x = 1;\n</script>", [][]byte{one("\nconst x = 1;\n")}},
		{"module", "<script context=\"module\">\nconst x = 1;\n</script>", [][]byte{one("\nconst x = 1;\n")}},
		{"genericsAttr", "<script lang=\"ts\" generics=\"T\">\nconst x = 1;\n</script>", [][]byte{one("\nconst x = 1;\n")}},
		{"both", "<script context=\"module\">\nconst a = 1;\n</script><script>\nconst b = 2;\n</script>", [][]byte{one("\nconst a = 1;\n"), one("\nconst b = 2;\n")}},
		{"upper", "<SCRIPT>\nconst x = 1;\n</SCRIPT>", [][]byte{one("\nconst x = 1;\n")}},
		{"closingFirst", "</script><script>\nconst x = 1;\n</script>", [][]byte{one("\nconst x = 1;\n")}},
		{"quotedAttr", "<script lang=\"a>b\">\nconst x = 1;\n</script>", [][]byte{one("\nconst x = 1;\n")}},
		{"singleQuotedAttr", "<script lang='a>b'>\nconst x = 1;\n</script>", [][]byte{one("\nconst x = 1;\n")}},
		{"scriptPrefix", "<scriptx>no</scriptx><script>\nconst x = 1;\n</script>", [][]byte{one("\nconst x = 1;\n")}},
		{"afterMarkup", "<div>t</div><custom-element foo=\"bar\"/><my_tag/><x:y/></template><script>\nconst x = 1;\n</script><script>\nconst y = 2;\n</script>", [][]byte{one("\nconst x = 1;\n"), one("\nconst y = 2;\n")}},
		{"commentedScript", "<!-- <script>import './fake.svelte';</script> --><script>\nconst x = 1;\n</script>", [][]byte{one("\nconst x = 1;\n")}},
		{"commentInScript", "<script>const x = 1;<!-- not html -->const y = 2;</script>", [][]byte{one("const x = 1;<!-- not html -->const y = 2;")}},
		{"selfClosingOnly", "<script/>", nil},
		{"selfClosingFirst", "<script/>\n<script>\nconst x = 1;\n</script>", [][]byte{one("\nconst x = 1;\n")}},
		{"partialSecondKeepsFirst", "<script>\nconst a = 1;\n</script><script>\nconst b = 2;", [][]byte{one("\nconst a = 1;\n")}},
		{"validThenUnclosedTag", "<script>\nconst a = 1;\n</script><script lang=\"ts\"", [][]byte{one("\nconst a = 1;\n")}},
		{"validThenUnterminatedComment", "<script>\nconst a = 1;\n</script><!-- <script>", [][]byte{one("\nconst a = 1;\n")}},
		{"validThenCommentInBrokenScript", "<script>\nconst a = 1;\n</script><script>x<!--", [][]byte{one("\nconst a = 1;\n")}},
		{"unclosedTag", "<script", nil},
		{"unclosedAttr", "<script lang=\"ts\"", nil},
		{"unterminatedAttrQuote", "<script lang=\"ts>", nil},
		{"noClose", "<script>\nconst x = 1;", nil},
		{"unterminatedComment", "<!-- <script>", nil},
		{"unterminatedCommentInScript", "<script>\nconst x = 1;<!--", nil},
	}
	for _, tc := range cases {
		for _, scope := range []ScriptScope{TopLevel, Anywhere} {
			if got := ExtractScripts([]byte(tc.source), scope); !reflect.DeepEqual(got, tc.all) {
				t.Errorf("ExtractScripts %s scope %d = %q, want %q", tc.name, scope, got, tc.all)
			}
		}
	}
}

func TestExtractScriptsScopes(t *testing.T) {
	one := func(s string) []byte { return []byte(s) }
	cases := []struct {
		name     string
		source   string
		top      [][]byte
		anywhere [][]byte
	}{
		{"vueSetupAndPlain", "<script>\nexport default {}\n</script>\n<script setup>\nimport A from './A.vue';\n</script>\n<template><A/></template>",
			[][]byte{one("\nexport default {}\n"), one("\nimport A from './A.vue';\n")},
			[][]byte{one("\nexport default {}\n"), one("\nimport A from './A.vue';\n")}},
		{"svelteModuleAndInstance", "<script module>\nimport a from './a';\n</script>\n<script>\nimport b from './b';\n</script>\n<p>x</p>",
			[][]byte{one("\nimport a from './a';\n"), one("\nimport b from './b';\n")},
			[][]byte{one("\nimport a from './a';\n"), one("\nimport b from './b';\n")}},
		{"nestedInTemplate", "<template><script>import x from './x';</script></template>",
			nil, [][]byte{one("import x from './x';")}},
		{"nestedInDiv", "<div><span><script>import x from './x';</script></span></div><script>import y from './y';</script>",
			[][]byte{one("import y from './y';")}, [][]byte{one("import x from './x';"), one("import y from './y';")}},
		{"styleBody", "<style>a::after { content: '<script>import x from \"./x\"</script>'; }</style><script>y</script>",
			[][]byte{one("y")}, [][]byte{one("y")}},
		{"textareaBody", "<textarea><script>x</script></textarea><script>y</script>",
			[][]byte{one("y")}, [][]byte{one("y")}},
		{"unclosedP", "<template><div><p>a<p>b</div></template><script>y</script>",
			[][]byte{one("y")}, [][]byte{one("y")}},
		{"voidElement", "<div><br><img src=x><input></div><script>y</script>",
			[][]byte{one("y")}, [][]byte{one("y")}},
		{"unquotedAttr", "<script lang=ts>y</script>",
			[][]byte{one("y")}, [][]byte{one("y")}},
		{"scriptDataEscape", "<script><!--<script></script>--></script><script>y</script>",
			[][]byte{one("<!--<script></script>-->"), one("y")}, [][]byte{one("<!--<script></script>-->"), one("y")}},
		{"strayEndTag", "</div></template><script>y</script>",
			[][]byte{one("y")}, [][]byte{one("y")}},
		{"vueInterpolationLessThan", "<template>{{ a<b }}</template><script>y</script>",
			[][]byte{one("y")}, [][]byte{one("y")}},
		{"vueInterpolationLoop", "<template><li v-for=\"i in n\">{{ i<n }}</li></template>\n<script setup>\nimport A from './A.vue';\n</script>",
			[][]byte{one("\nimport A from './A.vue';\n")}, [][]byte{one("\nimport A from './A.vue';\n")}},
		{"vuePugTemplate", "<template lang=\"pug\">div(a='<b')</template><script>y</script>",
			[][]byte{one("y")}, [][]byte{one("y")}},
		{"vueCustomBlock", "<i18n>{\"a\":\"<x\"}</i18n><script>y</script>",
			[][]byte{one("y")}, [][]byte{one("y")}},
		{"vueNestedTemplate", "<template><template v-if=\"a\"><script>x</script></template></template><script>y</script>",
			[][]byte{one("y")}, [][]byte{one("x"), one("y")}},
		{"vueCommentedEndTag", "<template><!-- </template> --><div><script>x</script></div></template><script>y</script>",
			[][]byte{one("y")}, [][]byte{one("x"), one("y")}},
		{"svelteIfAutoClosedP", "{#if a}<p>x{/if}<script>y</script>",
			[][]byte{one("y")}, [][]byte{one("y")}},
		// Vue/Svelte treat <div/> as self-closing, so it must not deepen the root block.
		{"selfClosingSameNameThenStrayEnd", "<div><div/></div>\n<script>y</script>\n<div></div></div>",
			[][]byte{one("y")}, [][]byte{one("y")}},
	}
	for _, tc := range cases {
		if got := ExtractScripts([]byte(tc.source), TopLevel); !reflect.DeepEqual(got, tc.top) {
			t.Errorf("%s TopLevel = %q, want %q", tc.name, got, tc.top)
		}
		if got := ExtractScripts([]byte(tc.source), Anywhere); !reflect.DeepEqual(got, tc.anywhere) {
			t.Errorf("%s Anywhere = %q, want %q", tc.name, got, tc.anywhere)
		}
	}
}

func TestExtractScriptsAliasesInput(t *testing.T) {
	src := []byte("<template>\r\n</template><script>\x00a\r\nb</script>")
	got := ExtractScripts(src, TopLevel)
	if len(got) != 1 || string(got[0]) != "\x00a\r\nb" {
		t.Fatalf("ExtractScripts = %q", got)
	}
	if &got[0][0] != &src[bytes.Index(src, []byte("\x00"))] {
		t.Error("region does not alias the input")
	}
}

func TestExtractScriptsNoPanic(t *testing.T) {
	seeds := []string{
		"<script>\xff\xfe</script>", "<\xff>", "<script \xed\xa0\x80>x</script>",
		"\xed\xa0\x80<script>", "<script>\"\\ud800\"</script", "<", "</", "<!", "<!--", "<![CDATA[",
		"<plaintext><script>x</script>", "<script/", "<a b='",
	}
	for _, s := range seeds {
		for _, scope := range []ScriptScope{TopLevel, Anywhere} {
			_ = ExtractScripts([]byte(s), scope)
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
		{"char literal stays on its line", "c = 'a\nimport a.B;\nd = 'b';\n", "c =   \nimport a.B;\nd =    ;\n"},
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
		{"interpolated verbatim reversed", "x = @$\"\nusing a.B;\n\";\n", "x =    \n          \n ;\n"},
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

func TestMaskNestedStyles(t *testing.T) {
	styles := map[string]func([]byte) []byte{"kotlin": MaskKotlinStyle, "scala": MaskScalaStyle}
	cases := []struct{ name, in, want string }{
		{"nested", "/* /* a */ b */\n", "               \n"},
		{"unterminated nested", "/* /* a */ import x\n", "                   \n"},
		{"block", "a /* b */ c\n", "a         c\n"},
		{"line", "a // b\n", "a     \n"},
		{"string inert", "val s = \"/* a\"\nimport x\n", "val s =       \nimport x\n"},
	}
	for name, mask := range styles {
		for _, tc := range cases {
			if got := string(mask([]byte(tc.in))); got != tc.want {
				t.Errorf("%s %s = %q, want %q", name, tc.name, got, tc.want)
			}
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
		{"type variable", "let f (x: 'a) = x\nopen A.B\nlet c = 'z'\n", "let f (x: 'a) = x\nopen A.B\nlet c =    \n"},
		{"generic type variable", "let m : Map<'T,'U> = x\nopen A.B\n", "let m : Map<'T,'U> = x\nopen A.B\n"},
		{"primed names", "let x' = 1\nlet y'' = 2\nopen A.B\n", "let x' = 1\nlet y'' = 2\nopen A.B\n"},
		{"quote char", "let c = '\"'\nopen A.B\n", "let c =    \nopen A.B\n"},
		{"newline escape", "let c = '\\n'\n", "let c =     \n"},
		{"decimal escape", "let c = '\\065'\n", "let c =       \n"},
		{"quote escape", "let c = '\\''\n", "let c =     \n"},
		{"multiply operator", "List.fold (*) 1 xs\nopen a.B\n", "List.fold (*) 1 xs\nopen a.B\n"},
		{"operator inside comment", "(* (*) open hidden.C *) open d.E\n", "                        open d.E\n"},
		{"empty comment", "(**) open d.E\n", "     open d.E\n"},
		{"unclosed stays", "let c = 'ab\nopen A.B\n", "let c = 'ab\nopen A.B\n"},
	}
	for _, tc := range cases {
		if got := string(MaskFSharpStyle([]byte(tc.in))); got != tc.want {
			t.Errorf("%s = %q, want %q", tc.name, got, tc.want)
		}
	}
}

func TestNormalizeDotted(t *testing.T) {
	std := func(name string) bool { return name == "java.util.List" }
	cases := []struct{ in, want string }{
		{"a.b.C", "C"},
		{"  a.b.C  ", "C"},
		{"C", "C"},
		{"", ""},
		{"   ", ""},
		{"java.util.List", "java.util.List"},
	}
	for _, tc := range cases {
		if got := NormalizeDotted(tc.in, std); got != tc.want {
			t.Errorf("NormalizeDotted(%q) = %q, want %q", tc.in, got, tc.want)
		}
	}
}

func TestDotnetPlatform(t *testing.T) {
	for _, spec := range []string{"System", "System.IO", "Microsoft.Win32", "System.Text.Json"} {
		if !IsDotnetPlatform(spec) {
			t.Errorf("IsDotnetPlatform(%q) = false, want true", spec)
		}
	}
	for _, spec := range []string{"System.IO.Abstractions", "System.CommandLine", "Microsoft.Extensions.Logging", "Acme", ""} {
		if IsDotnetPlatform(spec) {
			t.Errorf("IsDotnetPlatform(%q) = true, want false", spec)
		}
	}
	for _, spec := range []string{"FSharp.Core", "Microsoft.FSharp.Core", "FSharp.Collections", "Microsoft.FSharp.Collections.List"} {
		if !IsFSharpCorePlatform(spec) {
			t.Errorf("IsFSharpCorePlatform(%q) = false, want true", spec)
		}
	}
	for _, spec := range []string{"FSharp.Data.Runtime", "FSharp.Compiler.Text", "System"} {
		if IsFSharpCorePlatform(spec) {
			t.Errorf("IsFSharpCorePlatform(%q) = true, want false", spec)
		}
	}
}

func TestStripBOM(t *testing.T) {
	cases := []struct{ name, in, want string }{
		{"none", "abc", "abc"},
		{"one", "\xEF\xBB\xBFabc", "abc"},
		{"two", "\xEF\xBB\xBF\xEF\xBB\xBFabc", "\xEF\xBB\xBFabc"},
		{"only", "\xEF\xBB\xBF", ""},
		{"empty", "", ""},
		{"notAtStart", "a\xEF\xBB\xBF", "a\xEF\xBB\xBF"},
	}
	for _, tc := range cases {
		if got := string(StripBOM([]byte(tc.in))); got != tc.want {
			t.Errorf("%s: StripBOM(%q) = %q, want %q", tc.name, tc.in, got, tc.want)
		}
	}
}

func TestIncompleteJS(t *testing.T) {
	cases := []struct {
		src  string
		want bool
	}{
		{"", false},
		{"import a from './a';", false},
		{"import {", true},
		{"import {\n  a,\n} from './a';", false},
		{"export const x = [", true},
		{"export const x = f(", true},
		{"export const s = `a\n", true},
		{"export const s = `a ${b} c`;", false},
		{"export const s = `a ${ {b: 1} } c`;", false},
		{"export const s = `a ${", true},
		{"export const s = `\\``;", false},
		{"import a /* note", true},
		{"import a /* note */ from 'a';", false},
		{"import a from 'a'; // {", false},
		{"export const s = '{';", false},
		{"export const s = \"(\";", false},
		{"export const s = 'abc", false},
		{"export const r = /[{(]/;", false},
		{"export const x = a / b / c;", false},
		{"export const x = {a: [1, (2)]};", false},
		{"export default )", false},
	}
	for _, tc := range cases {
		if got := IncompleteJS([]byte(tc.src)); got != tc.want {
			t.Errorf("IncompleteJS(%q) = %v, want %v", tc.src, got, tc.want)
		}
	}
}
