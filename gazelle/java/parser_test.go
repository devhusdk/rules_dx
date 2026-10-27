package java

import (
	"reflect"
	"testing"
)

func TestParseImportsSingleType(t *testing.T) {
	got := ParseImports([]byte("package demo;\n\nimport com.example.Widget;\nimport java.util.List;\n"))
	want := []string{"com.example.Widget", "java.util.List"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsStaticAndOnDemand(t *testing.T) {
	got := ParseImports([]byte("package demo;\n\nimport static com.example.Widget.create;\nimport com.example.*;\n"))
	want := []string{"com.example.*", "com.example.Widget"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsCommentsAndStringsInert(t *testing.T) {
	got := ParseImports([]byte("package demo;\n\n// import com.fake.Commented;\n/* import com.fake.Blocked; */\nimport java.util.List;\n\nString s = \"import com.fake.Literal;\";\n"))
	want := []string{"java.util.List"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsDedupedSorted(t *testing.T) {
	got := ParseImports([]byte("package demo;\n\nimport com.example.Zeta;\nimport com.example.Alpha;\nimport com.example.Zeta;\n"))
	want := []string{"com.example.Alpha", "com.example.Zeta"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParsePackage(t *testing.T) {
	got, err := ParsePackage([]byte("package com.example.demo;\n\npublic class Demo {}\n"))
	if err != nil || got != "com.example.demo" {
		t.Errorf("ParsePackage = %q, %v; want com.example.demo", got, err)
	}
	if got, err := ParsePackage([]byte("public class Demo {}\n")); err != nil || got != "" {
		t.Errorf("ParsePackage default = %q, %v; want empty", got, err)
	}
	if _, err := ParsePackage([]byte("package a;\npackage b;\n")); err == nil {
		t.Fatal("ParsePackage succeeded on duplicate package clauses")
	}
}

func TestDefinesMain(t *testing.T) {
	if !DefinesMain([]byte("package demo;\n\npublic class Main {\n  public static void main(String[] args) {}\n}\n")) {
		t.Error("DefinesMain missed a main method")
	}
	if DefinesMain([]byte("package demo;\n\npublic class Demo {\n  // public static void main(String[] args)\n}\n")) {
		t.Error("DefinesMain matched a commented main")
	}
	if DefinesMain([]byte("package demo;\n\npublic class Demo {\n  public String hello() { return \"x\"; }\n}\n")) {
		t.Error("DefinesMain matched a library source")
	}
}

func TestIsStdLib(t *testing.T) {
	for _, imp := range []string{"java.util.List", "java.util", "javax.swing.JFrame", "javax.xml.parsers.DocumentBuilder", "jdk.internal.Foo", "org.w3c.dom.Node", "org.xml.sax.Parser"} {
		if !IsStdLib(imp) {
			t.Errorf("IsStdLib(%q) = false, want true", imp)
		}
	}
	for _, imp := range []string{"com.example.Widget", "Widget", "javax.inject.Inject", "javax.servlet.Servlet", "javax.annotation.Nullable", "javax.xml.bind.JAXBContext", "javafx.application.Application", "javaxfoo.Bar", "com.sun.jna.Native", "sun.foo.Bar"} {
		if IsStdLib(imp) {
			t.Errorf("IsStdLib(%q) = true, want false", imp)
		}
	}
}

func TestParsePackageDuplicateFails(t *testing.T) {
	if _, err := ParsePackage([]byte("package a;\npackage b;\n")); err == nil || err.Error() != "java: duplicate package clause a" {
		t.Fatalf("duplicate: %v", err)
	}
}

func TestParseImportsStaticDeclaringType(t *testing.T) {
	src := "package demo;\n\nimport static com.example.Widget.create;\nimport static com.example.Widget.MAX;\nimport static com.example.Widget.*;\nimport static com.example.Outer.Inner.build;\nimport static java.lang.Math.max;\nimport static Local.go;\n// import static com.fake.Commented.x;\n/* import static com.fake.Blocked.y; */\nimport com.example.Gadget;\n"
	got := ParseImports([]byte(src))
	want := []string{"Local", "com.example.Gadget", "com.example.Outer.Inner", "com.example.Widget", "java.lang.Math.max"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsWildcardStaysQualified(t *testing.T) {
	got := ParseImports([]byte("package demo;\n\nimport com.example.*;\nimport java.util.*;\nimport static com.example.Widget.*;\n"))
	want := []string{"com.example.*", "com.example.Widget", "java.util"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsTreeSitterShapes(t *testing.T) {
	tests := []struct {
		name, src string
		want      []string
	}{
		{"comment between segments", "import a./*x*/B;\n", []string{"a.B"}},
		{"multiline import", "import a.b\n    .C;\n", []string{"a.b.C"}},
		{"text block inert", "class A { String s = \"\"\"\nimport x.Y;\n\"\"\"; }\nimport k.L;\n", []string{"k.L"}},
		{"unicode identifiers", "import com.exämple.Wïdget;\n", []string{"com.exämple.Wïdget"}},
		{"truncated import", "import a.B;\nimport c.D;\nimport e.", []string{"a.B", "c.D"}},
		{"unclosed string after imports", "package p;\nimport a.B;\nimport c.D;\nclass X { String s = \"oops\n", []string{"a.B", "c.D"}},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := ParseImports([]byte(tt.src)); !reflect.DeepEqual(got, tt.want) {
				t.Errorf("ParseImports = %v, want %v", got, tt.want)
			}
		})
	}
}

func TestDefinesMainNestedClass(t *testing.T) {
	if !DefinesMain([]byte("class Outer {\n  static class Inner {\n    public static void main(String[] a) {}\n  }\n}\n")) {
		t.Error("DefinesMain missed a nested main")
	}
	if DefinesMain([]byte("class A { void main(String[] a) {} static int main() { return 0; } }\n")) {
		t.Error("DefinesMain matched a non-static or non-void main")
	}
}

func TestParseImportsSkipsModuleImport(t *testing.T) {
	got := ParseImports([]byte("import module java.base;\nimport a.B;\n"))
	if !reflect.DeepEqual(got, []string{"a.B"}) {
		t.Errorf("got %v, want [a.B]", got)
	}
}

func TestParseImportsKeepsPackageNamedModule(t *testing.T) {
	got := ParseImports([]byte("import module.Foo;\n"))
	if !reflect.DeepEqual(got, []string{"module.Foo"}) {
		t.Errorf("got %v, want [module.Foo]", got)
	}
}
