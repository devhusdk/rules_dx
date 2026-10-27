package common

import (
	"context"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
)

var webComponentCases = []struct {
	name, display, ext string
}{
	{"vue", "Vue", ".vue"},
	{"svelte", "Svelte", ".svelte"},
	{"astro", "Astro", ".astro"},
	{"mdx", "MDX", ".mdx"},
}

func noImports([]byte, string) []string { return nil }

func TestNewWebComponentLangMetadata(t *testing.T) {
	for _, tc := range webComponentCases {
		l := NewWebComponentLang(tc.name, tc.display, []string{tc.ext}, noImports)
		kind := tc.name + "_library"
		if l.Name() != tc.name || len(l.Kinds()) != 1 {
			t.Fatalf("%s: name %q kinds %v", tc.name, l.Name(), l.Kinds())
		}
		if _, ok := l.Kinds()[kind]; !ok {
			t.Errorf("%s: kinds %v lack %s", tc.name, l.Kinds(), kind)
		}
		loads := l.Loads()
		if len(loads) != 1 || loads[0].Name != "@rules_dx//"+tc.name+"/rules:defs.bzl" || strings.Join(loads[0].Symbols, ",") != kind {
			t.Errorf("%s: loads = %+v", tc.name, loads)
		}
	}
}

func TestWebComponentNaming(t *testing.T) {
	for _, tc := range webComponentCases {
		l := NewWebComponentLang(tc.name, tc.display, []string{tc.ext}, noImports)
		kind := tc.name + "_library"
		lib := rule.NewRule(kind, "x")
		lib.SetAttr("srcs", []string{"archive.tar" + tc.ext, "my-mod" + tc.ext, "notes.txt"})
		imports := l.Imports(&config.Config{}, lib, &rule.File{Pkg: "pkg"})
		var got []string
		for _, imp := range imports {
			got = append(got, imp.Imp)
		}
		if want := []string{"pkg/archive.tar", "pkg/my-mod"}; !reflect.DeepEqual(got, want) {
			t.Errorf("%s: imports = %q, want %q", tc.name, got, want)
		}

		generate := func(regular ...string) language.GenerateResult {
			root := t.TempDir()
			for _, name := range regular {
				dotFile(t, root, "pkg/demo/"+name, "")
			}
			return l.GenerateRules(language.GenerateArgs{
				Config:       &config.Config{RepoRoot: root},
				Dir:          filepath.Join(root, "pkg", "demo"),
				Rel:          "pkg/demo",
				RegularFiles: regular,
			})
		}
		l.Before(context.Background())
		var names []string
		for _, r := range generate("my-mod"+tc.ext, "Hello"+tc.ext, "notes.txt").Gen {
			if r.Kind() != kind {
				t.Errorf("%s: kind %s", tc.name, r.Kind())
			}
			names = append(names, r.Name())
		}
		if want := []string{"Hello", "my_mod"}; !reflect.DeepEqual(names, want) {
			t.Errorf("%s: targets = %q, want %q", tc.name, names, want)
		}
		if got := generate("---" + tc.ext).Gen; len(got) != 0 {
			t.Errorf("%s: generated %d rules for ---%s", tc.name, len(got), tc.ext)
		}
		func() {
			defer func() {
				msg, _ := recover().(string)
				if !strings.HasPrefix(msg, tc.display+" generation failed:") || !strings.Contains(msg, `"---" normalizes to an empty target name`) {
					t.Errorf("%s: panic = %q", tc.name, msg)
				}
			}()
			l.AfterResolvingDeps(context.Background())
		}()
	}
}

func vueWrap(script string) string {
	return "<template>\n  <div>hello</div>\n</template>\n\n<script>\n" + script + "</script>\n\n<style>\n.demo {\n  color: black;\n}\n</style>\n"
}

func TestParseScriptImportsVue(t *testing.T) {
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
		{"namespace", "import * as ns from \"../pkg/demo.vue\";\n", []string{"../pkg/demo"}},
		{"bare", "import React from \"react\";\n", []string{"react"}},
		{"vueBare", "import { ref } from \"vue\";\n", []string{"vue"}},
		{"scoped", "import x from \"@scope/pkg/sub\";\n", []string{"@scope/pkg/sub"}},
		{"scopedRoot", "import x from \"@scope/pkg\";\n", []string{"@scope/pkg"}},
		{"subpath", "import x from \"pkg/subpath\";\n", []string{"pkg/subpath"}},
		{"nodeBuiltin", "import fs from \"fs\";\n", []string{"fs"}},
		{"nodePrefix", "import fs from \"node:fs\";\n", []string{"node:fs"}},
		{"exportNamed", "export {a} from './helper.vue';\n", []string{"helper"}},
		{"exportStar", "export * from \"./demo.vue\";\n", []string{"demo"}},
		{"exportStarAs", "export * as ns from '../lib.vue';\n", []string{"../lib"}},
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
		{"twoScripts", "<script>\nimport a from './a.vue';\n</script>\n<script setup lang=\"ts\">\nimport b from './b.vue';\n</script>\n", []string{"a", "b"}},
		{"setupOnly", "<template><div>x</div></template>\n<script setup>\nimport b from './b.vue';\n</script>\n", []string{"b"}},
		{"unparseableSecondKeepsFirst", "<script>\nimport a from './a.vue';\n</script>\n<script setup>x", []string{"a"}},
	}
	for _, tc := range cases {
		var content []byte
		switch tc.name {
		case "empty", "noScript", "emptyScript", "selfClosingScript",
			"templateOnlyImport", "styleOnlyImport", "templateStyleInert",
			"twoScripts", "setupOnly", "unparseableSecondKeepsFirst":
			content = []byte(tc.source)
		default:
			content = []byte(vueWrap(tc.source))
		}
		if got := ParseScriptImports(content, ""); !reflect.DeepEqual(got, tc.want) {
			t.Errorf("%s: ParseScriptImports = %q, want %q", tc.name, got, tc.want)
		}
	}
}

func TestParseScriptImportsVueKeepsDirectories(t *testing.T) {
	content := []byte(vueWrap("import a from '../a/util.vue';\nimport b from '../b/util.vue';\nimport c from './util.vue';\nimport r from 'react';\n"))
	want := []string{"app/a/util", "app/b/util", "app/web/util", "react"}
	if got := ParseScriptImports(content, "app/web"); !reflect.DeepEqual(got, want) {
		t.Fatalf("ParseScriptImports = %q, want %q", got, want)
	}
}

func svelteWrap(script string) string {
	return "<script>\n" + script + "</script>\n\n<div class=\"hello\">hello</div>\n\n<style>\n.hello {\n  color: black;\n}\n</style>\n"
}

func svelteWrapModule(module, instance string) string {
	return "<script context=\"module\">\n" + module + "</script>\n\n<script>\n" + instance + "</script>\n\n<div class=\"hello\">hello</div>\n"
}

func TestParseScriptImportsSvelte(t *testing.T) {
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
		{"moduleAndInstance", svelteWrapModule("import a from './mod.svelte';\n", "import b from './inst.svelte';\n"), []string{"inst", "mod"}},
		{"moduleOnly", "<script context=\"module\">\nimport a from './mod.svelte';\n</script>\n<div>x</div>\n", []string{"mod"}},
		{"duplicateAcrossBlocks", svelteWrapModule("import a from './same.svelte';\n", "import b from './same.svelte';\n"), []string{"same"}},
		{"unparseableSecondKeepsFirst", "<script>\nimport a from './a.svelte';\n</script>\n<script>const x = 1;", []string{"a"}},
		{"sideEffect", "import \"./hello.svelte\";\n", []string{"hello"}},
		{"sideEffectSingle", "import './helper';\n", []string{"helper"}},
		{"default", "import hello from \"./hello.svelte\";\n", []string{"hello"}},
		{"named", "import {a, b} from './util.mjs';\n", []string{"util"}},
		{"namespace", "import * as ns from \"../pkg/demo.svelte\";\n", []string{"../pkg/demo"}},
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
		{"exportStarAs", "export * as ns from '../lib.svelte';\n", []string{"../lib"}},
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
			"unparseableSecondKeepsFirst":
			content = []byte(tc.source)
		default:
			content = []byte(svelteWrap(tc.source))
		}
		if got := ParseScriptImports(content, ""); !reflect.DeepEqual(got, tc.want) {
			t.Errorf("%s: ParseScriptImports = %q, want %q", tc.name, got, tc.want)
		}
	}
}

func TestParseScriptImportsSvelteKeepsDirectories(t *testing.T) {
	content := []byte(svelteWrap("import a from '../a/util.svelte';\nimport b from '../b/util.svelte';\nimport c from './util.svelte';\nimport r from 'react';\n"))
	want := []string{"app/a/util", "app/b/util", "app/web/util", "react"}
	if got := ParseScriptImports(content, "app/web"); !reflect.DeepEqual(got, want) {
		t.Fatalf("ParseScriptImports = %q, want %q", got, want)
	}
}
