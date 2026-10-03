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
