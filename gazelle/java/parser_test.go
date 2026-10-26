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
