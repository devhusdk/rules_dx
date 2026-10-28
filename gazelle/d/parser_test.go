package d

import (
	"reflect"
	"strings"
	"testing"
)

func TestParseImports(t *testing.T) {
	tests := []struct {
		name string
		src  string
		want []string
	}{
		{"plain", "import a.b;\n", []string{"a.b"}},
		{"multiple", "import a.b, c.d;\n", []string{"a.b", "c.d"}},
		{"selective", "import std.stdio : writeln, writefln;\nimport app.x : y, z = w;\n", []string{"app.x", "std.stdio"}},
		{"renamed", "import io = core.stdc.stdio;\n", []string{"core.stdc.stdio"}},
		{"static and public", "static import a.b;\npublic import c.d;\nprivate import e.f;\n", []string{"a.b", "c.d", "e.f"}},
		{"function scope", "void f() { import a.b; }\n", []string{"a.b"}},
		{"version", "version (linux) { import a.b; } else { import c.d; }\n", []string{"a.b", "c.d"}},
		{"static if", "static if (true) { import a.b; }\n", []string{"a.b"}},
		{"nested comment", "/+ outer /+ import fake.one; +/ import fake.two; +/\nimport real.one;\n", []string{"real.one"}},
		{"line and block comments", "// import fake.a;\n/* import fake.b; */\nimport real.one;\n", []string{"real.one"}},
		{"token string", "enum s = q{ import fake.two; };\nimport real.one;\n", []string{"real.one"}},
		{"mixin string", "mixin(\"import fake.three;\");\nimport real.one;\n", []string{"real.one"}},
		{"string import is not an import", "string t = import(\"data.txt\");\n", nil},
		{"duplicates", "import a.b;\nimport a.b;\n", []string{"a.b"}},
		{"bom and shebang", "\xef\xbb\xbf#!/usr/bin/env rdmd\nimport a.b;\n", []string{"a.b"}},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := ParseImports([]byte(tc.src))
			if !reflect.DeepEqual(got, tc.want) && (len(got) != 0 || len(tc.want) != 0) {
				t.Fatalf("ParseImports = %v, want %v", got, tc.want)
			}
		})
	}
}

func TestParseModule(t *testing.T) {
	tests := []struct{ name, src, want string }{
		{"declared", "module app.util;\n", "app.util"},
		{"doc comment first", "/** doc */ module app.util;\nimport a.b;\n", "app.util"},
		{"attribute", "@safe module app.util;\n", "app.util"},
		{"none", "import a.b;\n", ""},
		{"single segment", "module demo;\n", "demo"},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			if got := ParseModule([]byte(tc.src)); got != tc.want {
				t.Fatalf("ParseModule = %q, want %q", got, tc.want)
			}
		})
	}
}

func TestParseStringImports(t *testing.T) {
	got, err := ParseStringImports([]byte("string a = import(\"data.txt\");\nstring b = import(r\"raw.txt\");\nenum c = import(`tick.txt`);\n"))
	if err != nil {
		t.Fatal(err)
	}
	if want := []string{"data.txt", "raw.txt", "tick.txt"}; !reflect.DeepEqual(got, want) {
		t.Fatalf("ParseStringImports = %v, want %v", got, want)
	}
	for _, src := range []string{
		"string a = import(name);\n",
		"string a = import(\"a\" ~ \"b\");\n",
		"string a = import(\"a\\n\");\n",
	} {
		if _, err := ParseStringImports([]byte(src)); err == nil || !strings.Contains(err.Error(), "not a string literal") {
			t.Fatalf("ParseStringImports(%q) error = %v, want non-literal error", src, err)
		}
	}
}

func TestDefinesMain(t *testing.T) {
	tests := []struct {
		name string
		src  string
		want bool
	}{
		{"void", "void main() {}\n", true},
		{"int with args", "int main(string[] args) { return 0; }\n", true},
		{"extern C", "extern(C) int main(int argc, char** argv) { return 0; }\n", true},
		{"inside version", "version (linux) { void main() {} }\n", true},
		{"attribute block", "@safe { void main() {} }\n", true},
		{"struct method", "struct S { void main() {} }\nvoid f() {}\n", false},
		{"class method", "class C { void main() {} }\n", false},
		{"word in body", "void f() { auto main = 1; }\n", false},
		{"in unittest", "unittest { void main() {} }\n", false},
		{"comment", "// void main() {}\n", false},
		{"other name", "void mainly() {}\n", false},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			if got := DefinesMain([]byte(tc.src)); got != tc.want {
				t.Fatalf("DefinesMain = %v, want %v", got, tc.want)
			}
		})
	}
}

func TestHasUnittest(t *testing.T) {
	if !HasUnittest([]byte("unittest { assert(1 == 1); }\n")) {
		t.Fatal("unittest block not found")
	}
	if HasUnittest([]byte("// unittest {}\nvoid f() {}\n")) {
		t.Fatal("comment taken for a unittest")
	}
}

func TestAnalyzeReportsErrorLine(t *testing.T) {
	_, err := Analyze([]byte("module a;\n\nint x = 1 +;\n"))
	if err == nil || !strings.Contains(err.Error(), "line 3") {
		t.Fatalf("Analyze error = %v, want a line 3 syntax error", err)
	}
}

func TestIsForeign(t *testing.T) {
	tests := []struct {
		name string
		src  string
		want bool
	}{
		{"make deps", "build/foo.o: foo.c foo.h \\\n bar.h\n", true},
		{"make deps with path", "/abs/foo.o: foo.c\n", true},
		{"dtrace pragma", "#pragma D option quiet\nBEGIN { }\n", true},
		{"dtrace shebang", "#!/usr/sbin/dtrace -s\nBEGIN { }\n", true},
		{"dtrace provider", "provider app {\n  probe start(int);\n};\n", true},
		{"d module", "module a.b;\nimport std.stdio : writeln;\n", false},
		{"d selective import first", "import std.stdio : writeln;\n", false},
		{"d comment first", "// a.b: c\nmodule a;\n", false},
		{"rdmd script", "#!/usr/bin/env rdmd\nvoid main() {}\n", false},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			if got := IsForeign([]byte(tc.src)); got != tc.want {
				t.Fatalf("IsForeign = %v, want %v", got, tc.want)
			}
		})
	}
}

func TestIsStdLib(t *testing.T) {
	for _, name := range []string{"std.stdio", "core.stdc.stdio", "etc.c.zlib", "ldc.attributes", "object", "std"} {
		if !IsStdLib(name) {
			t.Errorf("IsStdLib(%q) = false", name)
		}
	}
	for _, name := range []string{"app.core", "objects", "stdx.thing", "app.std"} {
		if IsStdLib(name) {
			t.Errorf("IsStdLib(%q) = true", name)
		}
	}
}
