package kotlin

import (
	"reflect"
	"testing"
)

func TestParseImportsSingleType(t *testing.T) {
	got := ParseImports([]byte("package demo\n\nimport com.example.Widget\nimport java.util.List\n"))
	want := []string{"Widget", "java.util.List"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsAliasAndOnDemand(t *testing.T) {
	got := ParseImports([]byte("package demo\n\nimport com.example.Widget as W\nimport com.example.*\n"))
	want := []string{"Widget", "example"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsCommentsAndStringsInert(t *testing.T) {
	got := ParseImports([]byte("package demo\n\n// import com.fake.Commented\n/* import com.fake.Blocked */\nimport java.util.List\n\nval s = \"import com.fake.Literal\"\n"))
	want := []string{"java.util.List"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsDedupedSorted(t *testing.T) {
	got := ParseImports([]byte("package demo\n\nimport com.example.Zeta\nimport com.example.Alpha\nimport com.example.Zeta\n"))
	want := []string{"Alpha", "Zeta"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParsePackage(t *testing.T) {
	got, err := ParsePackage([]byte("package com.example.demo\n\nclass Demo\n"))
	if err != nil || got != "com.example.demo" {
		t.Errorf("ParsePackage = %q, %v; want com.example.demo", got, err)
	}
	if got, err := ParsePackage([]byte("class Demo\n")); err != nil || got != "" {
		t.Errorf("ParsePackage default = %q, %v; want empty", got, err)
	}
	if _, err := ParsePackage([]byte("package a\npackage b\n")); err == nil {
		t.Fatal("ParsePackage succeeded on duplicate package clauses")
	}
}

func TestDefinesMain(t *testing.T) {
	if !DefinesMain([]byte("package demo\n\nfun main() {}\n")) {
		t.Error("DefinesMain missed a main function")
	}
	if !DefinesMain([]byte("package demo\n\nfun main(args: Array<String>) {}\n")) {
		t.Error("DefinesMain missed a main function with args")
	}
	if DefinesMain([]byte("package demo\n\nclass Demo {\n  // fun main()\n}\n")) {
		t.Error("DefinesMain matched a commented main")
	}
	if DefinesMain([]byte("package demo\n\nclass Demo {\n  fun hello(): String { return \"x\" }\n}\n")) {
		t.Error("DefinesMain matched a library source")
	}
}

func TestIsStdLib(t *testing.T) {
	for _, imp := range []string{"kotlin.collections.List", "java.util.List", "java.util", "javax.swing.JFrame", "javax.xml.parsers.DocumentBuilder", "jdk.internal.Foo", "org.w3c.dom.Node", "org.xml.sax.Parser"} {
		if !IsStdLib(imp) {
			t.Errorf("IsStdLib(%q) = false, want true", imp)
		}
	}
	for _, imp := range []string{"com.example.Widget", "Widget", "javax.inject.Inject", "javax.servlet.Servlet", "javax.annotation.Nullable", "javax.xml.bind.JAXBContext", "javafx.application.Application", "javaxfoo.Bar", "com.sun.jna.Native", "sun.foo.Bar", "kotlinx.coroutines.runBlocking", "kotlinx.serialization.Serializable"} {
		if IsStdLib(imp) {
			t.Errorf("IsStdLib(%q) = true, want false", imp)
		}
	}
}

func TestParsePackageDuplicateFails(t *testing.T) {
	if _, err := ParsePackage([]byte("package a;\npackage b;\n")); err == nil || err.Error() != "kotlin: duplicate package clause a" {
		t.Fatalf("duplicate: %v", err)
	}
}
