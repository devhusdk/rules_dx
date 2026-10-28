package cc

import (
	"reflect"
	"testing"
)

func TestParseQuotedIncludesKeepsPath(t *testing.T) {
	got := ParseQuotedIncludes([]byte("#include <vector>\n#include \"cc/tests/fixtures/hello/hello.h\"\n#include \"./x.h\"\n"))
	want := []string{"cc/tests/fixtures/hello/hello.h", "x.h"}
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
	want := []string{"real/kept.h"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseQuotedIncludes = %v, want %v", got, want)
	}
}

func TestParseQuotedIncludesDedupedSorted(t *testing.T) {
	content := "#include \"zeta/z.h\"\n#include \"alpha/a.h\"\n#include \"zeta/z.h\"\n"
	got := ParseQuotedIncludes([]byte(content))
	want := []string{"alpha/a.h", "zeta/z.h"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseQuotedIncludes = %v, want %v", got, want)
	}
}

func TestParseQuotedIncludesEscapedQuoteInPath(t *testing.T) {
	got := ParseQuotedIncludes([]byte("#include \"dir/a\\\"b.h\"\n"))
	want := []string{"dir/a\\\"b.h"}
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
}

func TestPrefixedRawStrings(t *testing.T) {
	for _, prefix := range []string{"", "u8", "u", "U", "L"} {
		content := prefix + `R"x(` + "\n#include \"fake.h\"\n)x\";\n#include \"real.h\"\n"
		if got := ParseQuotedIncludes([]byte(content)); !reflect.DeepEqual(got, []string{"real.h"}) {
			t.Errorf("ParseQuotedIncludes(%q) = %v, want [real.h]", content, got)
		}
		src := prefix + `R"(int main() {})"`
		if DefinesMain([]byte(src)) {
			t.Errorf("DefinesMain(%q) = true, want false", src)
		}
	}
	for _, content := range []string{
		"fooLR\"x(\n#include \"a.h\"\n)x\"",
		"xu8R\"x(\n#include \"a.h\"\n)x\"",
	} {
		if got := ParseQuotedIncludes([]byte(content)); !reflect.DeepEqual(got, []string{"a.h"}) {
			t.Errorf("ParseQuotedIncludes(%q) = %v, want [a.h]", content, got)
		}
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
		"R\"x(\nint main() {}\n)x\"",
		"LR\"(int main() {})\"",
		"u8R\"x(\nint main() {}\n)x\"",
	} {
		if DefinesMain([]byte(content)) {
			t.Errorf("DefinesMain(%q) = true, want false", content)
		}
	}
}

func TestDigitSeparators(t *testing.T) {
	for _, num := range []string{"1'000", "0xAB'CD", "0b1010'0101", "1'000'000ull", "3.141'59f", "1'0e+1'0", ".5'0", "0x1p-1'0"} {
		content := "int n = " + num + ";\n#include \"lib.h\"\n"
		if got := ParseQuotedIncludes([]byte(content)); !reflect.DeepEqual(got, []string{"lib.h"}) {
			t.Errorf("ParseQuotedIncludes(%q) = %v, want [lib.h]", content, got)
		}
	}
	for _, content := range []string{
		"int n = 1'000;\nint main() { return 0; }\nchar c = 'x';\n",
		"int n = 1'000; int main() {}",
		"double d = .5'0; int main() {}",
	} {
		if !DefinesMain([]byte(content)) {
			t.Errorf("DefinesMain(%q) = false, want true", content)
		}
	}
	if !DefinesMain([]byte("auto c = u8'a'; int main() {}")) {
		t.Error("DefinesMain read u8'a' as a number")
	}
	if got := ParseQuotedIncludes([]byte("char c = u'\"';\n#include \"real.h\"")); !reflect.DeepEqual(got, []string{"real.h"}) {
		t.Errorf("prefixed char literal: %v", got)
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
	for directive, want := range map[string]string{"# include \\\n\"path/real.h\"": "path/real.h", "#\tinclude\\\r\n \"real.h\"": "real.h", " \t#\tinclude \"dir/real.h\"": "dir/real.h"} {
		if got := ParseQuotedIncludes([]byte(directive)); !reflect.DeepEqual(got, []string{want}) {
			t.Errorf("continued %q: %v, want [%s]", directive, got, want)
		}
	}
	if got := ParseQuotedIncludes([]byte("char c = 'a\n#include \"real.h\"")); !reflect.DeepEqual(got, []string{"real.h"}) {
		t.Errorf("unterminated char at newline: %v", got)
	}
	if !DefinesMain([]byte("char c = 'a\nint main() {}")) {
		t.Error("unterminated char masked the next line")
	}
	for _, literal := range []string{"/* main( unterminated", "\"main( unterminated\n", "'main( unterminated", "R\"main(\"", "\"escaped\\\" main(\"", `'\''`} {
		if DefinesMain([]byte(literal)) {
			t.Errorf("literal main: %q", literal)
		}
	}
}

func TestLexSourceSharedWalk(t *testing.T) {
	for _, c := range []struct {
		in           string
		wantIncludes []string
		wantMain     bool
	}{
		{"#include <main(x).h>", nil, false},
		{"#include \"main(.h\"", []string{"main(.h"}, false},
		{"int n = 1'000;\n#include \"a.h\"\nint main(){}", []string{"a.h"}, true},
		{"char c = 'a\n#include \"a.h\"\nint main(){}", []string{"a.h"}, true},
		{"u8R\"x(\n#include \"f.h\"\nint main(){}\n)x\"", nil, false},
		{"/* x\n#include \"f.h\" int main(){}", nil, false},
		{"int x; #include \"main(.h\"", nil, false},
	} {
		got := ParseQuotedIncludes([]byte(c.in))
		if len(got) != len(c.wantIncludes) || (len(got) > 0 && !reflect.DeepEqual(got, c.wantIncludes)) {
			t.Errorf("ParseQuotedIncludes(%q) = %v, want %v", c.in, got, c.wantIncludes)
		}
		if gotMain := DefinesMain([]byte(c.in)); gotMain != c.wantMain {
			t.Errorf("DefinesMain(%q) = %v, want %v", c.in, gotMain, c.wantMain)
		}
	}
	in := "int a; // c\n#include \\\n\"x/y.h\"\r\n/* b\nc */ char s[] = \"q\";\nR\"(\n)\" 'z';\n"
	joined := "int a; // c\n#include  \"x/y.h\"\r\n/* b\nc */ char s[] = \"q\";\nR\"(\n)\" 'z';\n"
	code, includes := lexSource([]byte(in))
	if !reflect.DeepEqual(includes, []string{"x/y.h"}) {
		t.Errorf("lexSource includes = %v, want [x/y.h]", includes)
	}
	if len(code) != len(joined) {
		t.Fatalf("len(code) = %d, want %d", len(code), len(joined))
	}
	for k := range joined {
		if (joined[k] == '\n') != (code[k] == '\n') {
			t.Errorf("newline mismatch at %d", k)
		}
	}
	if want := "int a;     \n                 \r\n    \n     char s[] =    ;\n   \n      ;\n"; string(code) != want {
		t.Errorf("lexSource code = %q, want %q", code, want)
	}
}

func TestParseSystemIncludesGuards(t *testing.T) {
	in := `#ifndef DEMO_H
#define DEMO_H
#include <stdio.h>
#include "local.h"
// #include <android/fake.h>
const char* s = R"(
#include <android/raw.h>
)";
#ifdef __ANDROID__
#include <android/log.h>
#else
#include <android/else.h>
#endif
#if defined(FOO)
#include <foo.h>
#elif defined(__ANDROID_API__)
#include <android/elif.h>
#endif
#include <android/native_activity.h>
#endif
`
	got := ParseSystemIncludes([]byte(in))
	want := []SystemInclude{
		{Path: "stdio.h"},
		{Path: "android/log.h", Guarded: true},
		{Path: "android/else.h", Guarded: true},
		{Path: "foo.h"},
		{Path: "android/elif.h", Guarded: true},
		{Path: "android/native_activity.h"},
	}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseSystemIncludes = %+v, want %+v", got, want)
	}
	if q := ParseQuotedIncludes([]byte(in)); !reflect.DeepEqual(q, []string{"local.h"}) {
		t.Errorf("ParseQuotedIncludes = %v, want [local.h]", q)
	}
}

func TestDefinesAndroidEntry(t *testing.T) {
	for _, c := range []struct {
		in   string
		want bool
	}{
		{"void ANativeActivity_onCreate(ANativeActivity* a, void* s, size_t n) {\n}\n", true},
		{"JNIEXPORT jint JNICALL JNI_OnLoad(JavaVM* vm, void* r)\n{\n  return 0;\n}\n", true},
		{"extern \"C\" jstring Java_org_example_Bridge_greet(JNIEnv* env, jobject) { return 0; }\n", true},
		{"void ANativeActivity_onCreate(ANativeActivity* a, void* s, size_t n);\n", false},
		{"jint JNI_OnLoad(JavaVM* vm, void* r);\n", false},
		{"// void JNI_OnLoad(JavaVM* vm) {}\n", false},
		{"const char* s = \"Java_x() {}\";\n", false},
		{"void MyJava_x() {}\n", false},
	} {
		if got := DefinesAndroidEntry([]byte(c.in)); got != c.want {
			t.Errorf("DefinesAndroidEntry(%q) = %v, want %v", c.in, got, c.want)
		}
	}
}
