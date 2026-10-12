package cc

import (
	"reflect"
	"testing"
)

func TestParseQuotedIncludesBasename(t *testing.T) {
	got := ParseQuotedIncludes([]byte("#include <vector>\n#include \"cc/tests/fixtures/hello/hello.h\"\n"))
	want := []string{"hello.h"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseQuotedIncludes = %v, want %v", got, want)
	}
}

func TestParseQuotedIncludesAngleInert(t *testing.T) {
	if got := ParseQuotedIncludes([]byte("#include <vector>\n#include <string>\n")); len(got) != 0 {
		t.Errorf("ParseQuotedIncludes angle-only = %v, want empty", got)
	}
}

func TestParseQuotedIncludesCommentsAndStringsInert(t *testing.T) {
	content := "// #include \"fake/comment.h\"\n/* #include \"fake/block.h\" */\n#include <string>\nconst char* s = \"#include \\\"fake/string.h\\\"\";\nchar c = '\"';\n#include \"real/kept.h\"\n"
	got := ParseQuotedIncludes([]byte(content))
	want := []string{"kept.h"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseQuotedIncludes = %v, want %v", got, want)
	}
}

func TestParseQuotedIncludesDedupedSorted(t *testing.T) {
	content := "#include \"zeta/z.h\"\n#include \"alpha/a.h\"\n#include \"zeta/z.h\"\n"
	got := ParseQuotedIncludes([]byte(content))
	want := []string{"a.h", "z.h"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseQuotedIncludes = %v, want %v", got, want)
	}
}

func TestParseQuotedIncludesEscapedQuoteInPath(t *testing.T) {
	got := ParseQuotedIncludes([]byte("#include \"dir/a\\\"b.h\"\n"))
	want := []string{"a\\\"b.h"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseQuotedIncludes escaped quote = %v, want %v", got, want)
	}
}

func TestParseQuotedIncludesMalformedRawStringsInert(t *testing.T) {
	for _, inert := range []string{
		`R"de lim(x)de lim"`,
		`R"abc`,
		`R"(hello`,
	} {
		if got := ParseQuotedIncludes([]byte(inert)); len(got) != 0 {
			t.Errorf("inert %q: %v", inert, got)
		}
	}
	if end, ok := skipRawString([]byte(`R"(hello`), 0); !ok || end != len(`R"(hello`) {
		t.Errorf("skipRawString unterminated = (%d, %v), want (%d, true)", end, ok, len(`R"(hello`))
	}
}

func TestDefinesMain(t *testing.T) {
	if !DefinesMain([]byte("#include <iostream>\nint main() { return 0; }\n")) {
		t.Error("DefinesMain missed a main definition")
	}
	if !DefinesMain([]byte("int\nmain (int argc, char** argv) { return 0; }\n")) {
		t.Error("DefinesMain missed a split-line main")
	}
	for _, content := range []string{
		"int domain() { return 0; }\n",
		"// int main() {}\n",
		"/* int main() {} */\n",
		"const char* s = \"int main() {}\";\n",
		"int Add(int a, int b) { return a + b; }\n",
	} {
		if DefinesMain([]byte(content)) {
			t.Errorf("DefinesMain(%q) = true, want false", content)
		}
	}
}

func TestMalformedIncludesAndLiteralBoundaries(t *testing.T) {
	for _, inert := range []string{
		"#", "#include", "#include MACRO", "#included \"fake.h\"", "int x; #include \"fake.h\"",
		"#include \"unterminated", "#include <unterminated", "#include \"\"", "/* unterminated\n#include \"fake.h\"",
		"\"unterminated\n", "'unterminated", "\"escaped\\\"quote\"", `'\''`, "R\"tag(\n#include \"fake.h\"\n)tag\"",
	} {
		if got := ParseQuotedIncludes([]byte(inert)); len(got) != 0 {
			t.Errorf("inert %q: %v", inert, got)
		}
	}
	for _, directive := range []string{"# include \\\n\"path/real.h\"", "#\tinclude\\\r\n \"real.h\"", " \t#\tinclude \"dir/real.h\""} {
		if got := ParseQuotedIncludes([]byte(directive)); !reflect.DeepEqual(got, []string{"real.h"}) {
			t.Errorf("continued %q: %v", directive, got)
		}
	}
	for _, literal := range []string{"/* main( unterminated", "\"main( unterminated\n", "'main( unterminated", "R\"main(\"", "\"escaped\\\" main(\"", `'\''`} {
		if DefinesMain([]byte(literal)) {
			t.Errorf("literal main: %q", literal)
		}
	}
	if _, _, ok := parseIncludePath([]byte("other"), 0); ok {
		t.Fatal("non-include parsed")
	}
}
