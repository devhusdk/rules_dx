package csharp

import (
	"reflect"
	"testing"
)

func TestParseImportsSingleType(t *testing.T) {
	got := ParseImports([]byte("namespace Demo;\n\nusing Acme.Widget;\nusing System.Collections.Generic;\n"))
	want := []string{"System.Collections.Generic", "Widget"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsAliasAndStatic(t *testing.T) {
	got := ParseImports([]byte("namespace Demo;\n\nusing static System.Math;\nusing W = Acme.Widget;\n"))
	want := []string{"System.Math", "Widget"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsCommentsAndStringsInert(t *testing.T) {
	got := ParseImports([]byte("namespace Demo;\n\n// using Acme.Commented;\n/* using Acme.Blocked; */\nusing System.Text;\n\nvar s = \"using Acme.Literal;\";\n"))
	want := []string{"System.Text"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsDedupedSorted(t *testing.T) {
	got := ParseImports([]byte("namespace Demo;\n\nusing Acme.Zeta;\nusing Acme.Alpha;\nusing Acme.Zeta;\n"))
	want := []string{"Alpha", "Zeta"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsMultiLineLiteralsInert(t *testing.T) {
	for _, source := range []string{
		"namespace Demo;\n\nclass C { string s = @\"\nusing Ghost.Phantom;\n\"; }\n",
		"namespace Demo;\n\nclass C { string s = $@\"\nusing Ghost.Phantom;\n\"; }\n",
		"namespace Demo;\n\nclass C { string s = \"\"\"\nusing Ghost.Phantom;\n\"\"\"; }\n",
		"namespace Demo;\n\nclass C { string s = @\"\nusing Ghost.Phantom;\n",
	} {
		if got := ParseImports([]byte(source)); len(got) != 0 {
			t.Errorf("inert %q: %v", source, got)
		}
	}
}

func TestParseImportsKeepsCodeAfterMultiLineLiteral(t *testing.T) {
	got := ParseImports([]byte("namespace Demo;\n\nclass C {\n  string s = @\"\nusing Ghost.Phantom;\n\";\n  void Run() { }\n}\n\nusing Acme.Widget;\n"))
	want := []string{"Widget"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParsePackage(t *testing.T) {
	got, err := ParsePackage([]byte("namespace Acme.Demo;\n\nclass Demo {}\n"))
	if err != nil || got != "Acme.Demo" {
		t.Errorf("ParsePackage = %q, %v; want Acme.Demo", got, err)
	}
	if got, err := ParsePackage([]byte("namespace Demo;\n\nclass Demo {}\n")); err != nil || got != "Demo" {
		t.Errorf("ParsePackage file-scoped = %q, %v; want Demo", got, err)
	}
	if got, err := ParsePackage([]byte("class Demo {}\n")); err != nil || got != "" {
		t.Errorf("ParsePackage default = %q, %v; want empty", got, err)
	}
	if _, err := ParsePackage([]byte("namespace A {}\nnamespace B {}\n")); err == nil {
		t.Fatal("ParsePackage succeeded on duplicate namespace declarations")
	}
}

func TestDefinesMain(t *testing.T) {
	if !DefinesMain([]byte("namespace Demo;\n\nclass Main {\n  static void Main(string[] args) {}\n}\n")) {
		t.Error("DefinesMain missed a main function")
	}
	if DefinesMain([]byte("namespace Demo;\n\nclass Demo {\n  // static void Main()\n}\n")) {
		t.Error("DefinesMain matched a commented main")
	}
	if DefinesMain([]byte("namespace Demo;\n\nclass Demo {\n  void Hello() {}\n}\n")) {
		t.Error("DefinesMain matched a library source")
	}
}

func TestIsStdLib(t *testing.T) {
	for _, imp := range []string{"System.Collections.Generic", "System.Text", "Microsoft.Extensions.Logging", "System", "Microsoft"} {
		if !IsStdLib(imp) {
			t.Errorf("IsStdLib(%q) = false, want true", imp)
		}
	}
	for _, imp := range []string{"Acme.Widget", "Widget"} {
		if IsStdLib(imp) {
			t.Errorf("IsStdLib(%q) = true, want false", imp)
		}
	}
}

func TestTruncatedLiteralsAndCommentsAreInert(t *testing.T) {
	for _, source := range []string{"/*\nusing Fake.Block;\n*/", "/*\nusing Fake.Unterminated;", "\"escaped\\\" using Fake.String;\"", "\"unterminated\n", "'x'", `'\''`, "'unterminated", "'unterminated\n"} {
		if got := ParseImports([]byte(source)); len(got) != 0 {
			t.Errorf("inert %q: %v", source, got)
		}
		if DefinesMain([]byte(source)) {
			t.Errorf("unexpected entry point: %q", source)
		}
	}
	if got := normalizeImport(" "); got != "" {
		t.Fatalf("empty import: %q", got)
	}
	if _, err := ParsePackage([]byte("namespace A {}\nnamespace B {}\n")); err == nil || err.Error() != "csharp: duplicate namespace declaration A" {
		t.Fatalf("duplicate: %v", err)
	}
}
