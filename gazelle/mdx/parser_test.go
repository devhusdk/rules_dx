package mdx

import (
	"reflect"
	"strings"
	"testing"
)

func mdxWrap(esm string) string {
	return esm + "\n# Demo\n\nContent.\n"
}

func TestParseImports(t *testing.T) {
	cases := []struct {
		name   string
		source string
		want   []string
	}{
		{"empty", "", nil},
		{"proseOnly", "Hello world\n", nil},
		{"proseParagraph", "Some prose\nmore prose\n", nil},
		{"importAtStart", "import helper from \"./helper.mdx\";\n", []string{"helper"}},
		{"importAfterBlank", "Some text\n\nimport helper from \"./helper.mdx\";\n", []string{"helper"}},
		{"importParagraphContinuation", "Some text\nimport helper from \"./helper.mdx\";\n", nil},
		{"importAfterHeading", "# Demo\nimport helper from \"./helper.mdx\";\n", []string{"helper"}},
		{"importAfterThematicBreak", "---\nimport helper from \"./helper.mdx\";\n", []string{"helper"}},
		{"importAfterSetext", "Title\n===\nimport helper from \"./helper.mdx\";\n", []string{"helper"}},
		{"importAfterFence", "```js\nimport fake from \"./fake.mdx\";\n```\nimport helper from \"./helper.mdx\";\n", []string{"helper"}},
		{"fencedCodeInert", "```js\nimport fake from \"./fake.mdx\";\n```\n", nil},
		{"tildeFenceInert", "~~~js\nimport fake from \"./fake.mdx\";\n~~~\n", nil},
		{"indentedImportInert", "    import helper from \"./helper.mdx\";\n", nil},
		{"blockquoteInert", "> import helper from \"./helper.mdx\";\n", nil},
		{"listInert", "- import helper from \"./helper.mdx\";\n", nil},
		{"singleLineHtmlInert", "<div>import helper from \"./helper.mdx\";</div>\n", nil},
		{"htmlCommentInlineInert", "<!-- import helper from \"./helper.mdx\"; -->\n", nil},
		{"htmlCommentBlockInert", "<!--\nimport helper from \"./helper.mdx\";\n-->\n", nil},
		{"unterminatedHtmlCommentWholeDocInert", "import a from \"./a.mdx\";\n<!-- unterminated\nimport b from \"./b.mdx\";\n", nil},
		{"jsxInert", "<Component prop=\"import './fake.mdx'\" />\n", nil},
		{"sideEffect", "import \"./hello.mdx\";\n", []string{"hello"}},
		{"sideEffectSingle", "import './helper';\n", []string{"helper"}},
		{"default", "import hello from \"./hello.mdx\";\n", []string{"hello"}},
		{"named", "import {a, b} from './util.mjs';\n", []string{"util"}},
		{"namespace", "import * as ns from \"../pkg/demo.mdx\";\n", []string{"../pkg/demo"}},
		{"bare", "import React from \"react\";\n", []string{"react"}},
		{"mdxBare", "import { compile } from \"@mdx-js/mdx\";\n", []string{"@mdx-js/mdx"}},
		{"scoped", "import x from \"@scope/pkg/sub\";\n", []string{"@scope/pkg/sub"}},
		{"scopedRoot", "import x from \"@scope/pkg\";\n", []string{"@scope/pkg"}},
		{"subpath", "import x from \"pkg/subpath\";\n", []string{"pkg/subpath"}},
		{"nodeBuiltin", "import fs from \"fs\";\n", []string{"fs"}},
		{"nodePrefix", "import fs from \"node:fs\";\n", []string{"node:fs"}},
		{"exportNamed", "export {a} from './helper.mdx';\n", []string{"helper"}},
		{"exportStar", "export * from \"./demo.mdx\";\n", []string{"demo"}},
		{"exportStarAs", "export * as ns from '../lib.mdx';\n", []string{"../lib"}},
		{"exportLocal", "export const x = 1;\n", nil},
		{"dynamicConstInert", "const m = await import(\"./lazy.mdx\");\n", nil},
		{"dynamicSingleProse", "import('./other.mjs');\n", nil},
		{"dynamicBareProse", "import(\"react\");\n", nil},
		{"requireConstInert", "const x = require(\"./data.cjs\");\n", nil},
		{"requireBareLineInert", "require('fs');\n", nil},
		{"requireComputed", "require(name);\n", nil},
		{"dynamicComputed", "import(name);\n", nil},
		{"templateDynamic", "import(`./lazy.mdx`);\n", nil},
		{"importMeta", "const u = import.meta.url;\n", nil},
		{"methodRequire", "obj.require(\"./fake.mdx\");\n", nil},
		{"noImports", "const x = 1;\n", nil},
		{"lineComment", "// import foo from \"bar\";\n", nil},
		{"blockComment", "/* import \"./hidden.mdx\"; */\nconst x = 1;\n", nil},
		{"trailingComment", "import a from \"./real.mdx\"; // import \"./fake.mdx\";\n", []string{"real"}},
		{"doubleStringProse", "\"import './fake.mdx'\";\nimport y from './real.mdx';\n", nil},
		{"singleString", "'require(\"./fake.mdx\")';\n", nil},
		{"templateInert", "`import './fake.mdx'`;\n", nil},
		{"regexInert", "const re = /import './fake.mdx'/;\nconst x = 1;\n", nil},
		{"unterminatedProse", "\"abc\nimport y from './real.mdx';\n", nil},
		{"relativeNoExt", "import x from './helper';\n", []string{"helper"}},
		{"relativeDir", "import x from './dir/';\n", []string{"dir"}},
		{"multiline", "import {\n a,\n b\n} from './multi.mdx';\n", []string{"multi"}},
		{"prettierWrapped", "import {\n  Chart,\n  Table,\n} from './widgets';\n", []string{"widgets"}},
		{"wrappedThenImport", "import {\n  a,\n} from './one.mdx';\n\nimport b from './two.mdx';\n", []string{"one", "two"}},
		{"exportMultiline", "export {\na\n} from \"./shared.mdx\";\n", []string{"shared"}},
		{"duplicate", "import a from './same.mdx';\nimport b from './same.mdx';\n", []string{"same"}},
		{"emptySpec", "import \"\";\n", nil},
		{"unterminatedBlock", "/* import \"./hidden.mdx\";", nil},
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
		{"exportNewlineFrom", "export {a}\nfrom './helper.mdx';\n", []string{"helper"}},
		{"exportNewlineNoFrom", "export const x = 1\n", nil},
		{"exportFromUnterminated", "export {a} from \"abc\n", nil},
		{"exportFromNonQuote", "export {a} from bar;\n", nil},
		{"exportEOF", "export", nil},
		{"exportBraceEOF", "export {", nil},
		{"requireNoParen", "require;\n", nil},
		{"requireEOF", "require", nil},
		{"requireMissingParen", "require(\"a\";\n", nil},
		{"requireUnterminated", "require(\"abc\n", nil},
		{"triviaLineComment", "import //c\n\"./a.mdx\";\n", []string{"a"}},
		{"triviaBlockComment", "import /*c*/ \"./a.mdx\";\n", []string{"a"}},
		{"triviaUnterminatedBlock", "import /* unterminated", nil},
		{"importBraceComment", "import {a /*c*/} from './real.mdx';\n", []string{"real"}},
		{"exportBraceComment", "export {a /*c*/} from './real.mdx';\n", []string{"real"}},
		{"inertEscapeProse", "\"a\\nb\";\nimport y from './real.mdx';\n", nil},
		{"templateInterpObjectProse", "`outer ${ {a: 1} } inner`;\nimport y from './real.mdx';\n", nil},
		{"quotedEscape", "import \"a\\\"b\";\n", []string{"a\"b"}},
		{"quotedBackslashEOF", "import \"abc\\", nil},
		{"quotedEOF", "import \"abc", nil},
		{"templateEscapeProse", "`a\\nb`;\nimport y from './real.mdx';\n", nil},
		{"templateInterpProse", "`outer ${x} inner`;\nimport y from './real.mdx';\n", nil},
		{"templateInterpQuoteProse", "`outer ${'x'} inner`;\nimport y from './real.mdx';\n", nil},
		{"templateInterpNestedProse", "`outer ${`inner`} end`;\nimport y from './real.mdx';\n", nil},
		{"templateUnterminated", "`abc", nil},
		{"regexAtStartProse", "/abc/;\nimport y from './real.mdx';\n", nil},
		{"divisionProse", "const x = a / b;\nimport y from './real.mdx';\n", nil},
		{"divisionParenProse", "const x = (a) / b;\nimport y from './real.mdx';\n", nil},
		{"regexEscapeProse", "const re = /a\\/b/;\nimport y from './real.mdx';\n", nil},
		{"regexNewlineProse", "const re = /abc\nimport y from './real.mdx';\n", nil},
		{"regexClassProse", "const re = /[a/b]/;\nimport y from './real.mdx';\n", nil},
		{"regexEOF", "const re = /abc", nil},
		{"importBadChar", "importx from \"./fake.mdx\";\n", nil},
		{"exportBadChar", "exportx from \"./fake.mdx\";\n", nil},
		{"bareHeadingBoundary", "###\nimport helper from \"./helper.mdx\";\n", []string{"helper"}},
		{"blankInsideBraces", "import {\n\n} from \"./a.mdx\";\n", []string{"a"}},
		{"continuationTwoLine", "import {\n} from \"./a.mdx\";\n", []string{"a"}},
		{"fenceIndentedCloser", "```js\nimport fake from \"./fake.mdx\";\n    ```\nimport helper from \"./helper.mdx\";\n", nil},
		{"fenceSpacedCloser", "```js\nimport fake from \"./fake.mdx\";\n ```\nimport helper from \"./helper.mdx\";\n", []string{"helper"}},
		{"fenceShortCloser", "````\nimport fake from \"./fake.mdx\";\n```\nimport helper from \"./helper.mdx\";\n", nil},
		{"fenceTrailingText", "```js\nimport fake from \"./fake.mdx\";\n``` extra\nimport helper from \"./helper.mdx\";\n", nil},
	}
	raw := map[string]bool{
		"empty": true, "proseOnly": true, "proseParagraph": true,
		"importAtStart": true, "importAfterBlank": true,
		"importParagraphContinuation": true, "importAfterHeading": true,
		"importAfterThematicBreak": true, "importAfterSetext": true,
		"importAfterFence": true, "fencedCodeInert": true,
		"tildeFenceInert": true, "indentedImportInert": true,
		"blockquoteInert": true, "listInert": true,
		"singleLineHtmlInert": true, "htmlCommentInlineInert": true,
		"htmlCommentBlockInert":                true,
		"unterminatedHtmlCommentWholeDocInert": true,
		"jsxInert":                             true,
		"bareHeadingBoundary":                  true,
		"fenceIndentedCloser":                  true, "fenceSpacedCloser": true,
		"fenceShortCloser": true, "fenceTrailingText": true,
	}
	for _, tc := range cases {
		var content []byte
		if raw[tc.name] {
			content = []byte(tc.source)
		} else {
			content = []byte(mdxWrap(tc.source))
		}
		if got := ParseImports(content, ""); !reflect.DeepEqual(got, tc.want) {
			t.Errorf("%s: ParseImports = %q, want %q", tc.name, got, tc.want)
		}
	}
}

func TestExtractESMRegions(t *testing.T) {
	cases := []struct {
		name   string
		source string
		want   []string
	}{
		{"empty", "", nil},
		{"proseOnly", "Hello\n", nil},
		{"singleImport", "import a from \"./a.mdx\";\n", []string{"import a from \"./a.mdx\";\n"}},
		{"blankSeparated", "import a from \"./a.mdx\";\n\nimport b from \"./b.mdx\";\n", []string{"import a from \"./a.mdx\";\n", "import b from \"./b.mdx\";\n"}},
		{"paragraphContinuationInert", "text\nimport a from \"./a.mdx\";\n", nil},
		{"headingBoundary", "# T\nimport a from \"./a.mdx\";\n", []string{"import a from \"./a.mdx\";\n"}},
		{"bareHeadingBoundary", "###\nimport a from \"./a.mdx\";\n", []string{"import a from \"./a.mdx\";\n"}},
		{"thematicBoundary", "---\nimport a from \"./a.mdx\";\n", []string{"import a from \"./a.mdx\";\n"}},
		{"fenceInert", "```js\nimport a from \"./a.mdx\";\n```\n", nil},
		{"fenceSpacedCloser", "```js\nimport a from \"./a.mdx\";\n ```\nimport b from \"./b.mdx\";\n", []string{"import b from \"./b.mdx\";\n"}},
		{"fenceIndentedCloserInert", "```js\nimport a from \"./a.mdx\";\n    ```\nimport b from \"./b.mdx\";\n", nil},
		{"fenceShortCloserInert", "````\nimport a from \"./a.mdx\";\n```\nimport b from \"./b.mdx\";\n", nil},
		{"fenceTrailingTextInert", "```js\nimport a from \"./a.mdx\";\n``` extra\nimport b from \"./b.mdx\";\n", nil},
		{"blankInsideBraces", "import {\n\n} from \"./a.mdx\";\n", []string{"import {\n\n} from \"./a.mdx\";\n"}},
		{"continuationTwoLine", "import {\n} from \"./a.mdx\";\n", []string{"import {\n} from \"./a.mdx\";\n"}},
		{"badImportOpener", "importx from \"./a.mdx\";\n", nil},
		{"badExportOpener", "exportx from \"./a.mdx\";\n", nil},
		{"multilineBraces", "import {\n a\n} from \"./a.mdx\";\n", []string{"import {\n a\n} from \"./a.mdx\";\n"}},
		{"headingInsideOpenBrace", "import {\n# Title\n", []string{"import {\n# Title\n"}},
		{"commentWholeDocInert", "import a from \"./a.mdx\";\n<!-- oops\nimport b from \"./b.mdx\";\n", nil},
		{"tildeFence", "~~~\nimport a from \"./a.mdx\";\n~~~\nimport b from \"./b.mdx\";\n", []string{"import b from \"./b.mdx\";\n"}},
		{"longerCloser", "```\nimport a from \"./a.mdx\";\n`````\nimport b from \"./b.mdx\";\n", []string{"import b from \"./b.mdx\";\n"}},
		{"tildeCloserForBacktickInert", "```\nimport a from \"./a.mdx\";\n~~~\nimport b from \"./b.mdx\";\n", nil},
		{"fenceToEOF", "import a from \"./a.mdx\";\n\n```\nimport b from \"./b.mdx\";\n", []string{"import a from \"./a.mdx\";\n"}},
		{"fenceInList", "- item\n\n  ```\n  import a from \"./a.mdx\";\n  ```\n\nimport b from \"./b.mdx\";\n", []string{"import b from \"./b.mdx\";\n"}},
		{"fenceInBlockquote", "> ```\n> import a from \"./a.mdx\";\n> ```\n\nimport b from \"./b.mdx\";\n", []string{"import b from \"./b.mdx\";\n"}},
		{"blockquoteInert", "> import a from \"./a.mdx\";\n", nil},
		{"listItemInert", "- import a from \"./a.mdx\";\n", nil},
		{"listContinuationInert", "- item\n\n  import a from \"./a.mdx\";\n", nil},
		{"indentedImportInert", "    import a from \"./a.mdx\";\n", nil},
		{"spacedImportInert", " import a from \"./a.mdx\";\n", nil},
		{"tabAfterKeywordInert", "import\ta from \"./a.mdx\";\n", nil},
		{"braceAfterKeywordInert", "import{a} from \"./a.mdx\";\n", nil},
		{"threeBlocks", "import a from \"./a.mdx\";\n\nProse.\n\nexport {b} from \"./b.mdx\";\n\n# Title\n\nimport c from \"./c.mdx\";\n", []string{"import a from \"./a.mdx\";\n", "export {b} from \"./b.mdx\";\n", "import c from \"./c.mdx\";\n"}},
		{"blankInsideTemplate", "export const s = `a\n\nb`;\nimport a from \"./a.mdx\";\n\nText\n", []string{"export const s = `a\n\nb`;\nimport a from \"./a.mdx\";\n"}},
		{"blankEndsComplete", "import a from \"./a.mdx\";\n\nimport(\"./b.mdx\");\n", []string{"import a from \"./a.mdx\";\n"}},
		{"setextAfterImport", "import a from \"./a.mdx\";\n===\n", []string{"import a from \"./a.mdx\";\n===\n"}},
		{"thematicAfterImport", "import a from \"./a.mdx\";\n---\n", []string{"import a from \"./a.mdx\";\n---\n"}},
		{"crlf", "import a from \"./a.mdx\";\r\n\r\nText\r\n", []string{"import a from \"./a.mdx\";\r\n"}},
	}
	for _, tc := range cases {
		got := extractESMRegions([]byte(tc.source))
		if tc.want == nil {
			if got != nil {
				t.Errorf("%s: extractESMRegions = %q, want nil", tc.name, got)
			}
			continue
		}
		if len(got) != len(tc.want) {
			t.Errorf("%s: extractESMRegions = %q, want %q", tc.name, got, tc.want)
			continue
		}
		for i := range got {
			if strings.TrimSpace(string(got[i])) != strings.TrimSpace(tc.want[i]) {
				t.Errorf("%s: region[%d] = %q, want %q", tc.name, i, got[i], tc.want[i])
			}
		}
	}
}

func TestParseImportsKeepsDirectories(t *testing.T) {
	content := []byte(mdxWrap("import a from '../a/util.mdx';\nimport b from '../b/util.mdx';\nimport c from './util.mdx';\nimport r from 'react';\n"))
	want := []string{"app/a/util", "app/b/util", "app/web/util", "react"}
	if got := ParseImports(content, "app/web"); !reflect.DeepEqual(got, want) {
		t.Fatalf("ParseImports = %q, want %q", got, want)
	}
}

func TestExtractESMRegionsNoPanic(t *testing.T) {
	seeds := []string{
		"import \xff\xfe from 'a';\n", "\xed\xa0\x80import a from 'a';\n", "import a from '\xed\xa0\x80';\n",
		"import ", "export ", "import {", "```", "> ", "- ", "<!--", "import `${", "\x00import a\n",
	}
	for _, s := range seeds {
		_ = extractESMRegions([]byte(s))
	}
}
