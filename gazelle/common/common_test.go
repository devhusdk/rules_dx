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
