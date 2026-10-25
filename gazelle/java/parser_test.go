package java

import (
	"reflect"
	"strings"
	"testing"
)

func parse(t *testing.T, src string) ([]string, string, bool, []string) {
	t.Helper()
	facts, err := ParseSource([]byte(src))
	if err != nil {
		t.Fatalf("ParseSource: %v", err)
	}
	return facts.Imports, facts.Package, facts.Main, facts.Declared
}

func TestParseImportsKeepQualifiedNames(t *testing.T) {
	got, _, _, _ := parse(t, "package demo;\n\nimport com.example.Widget;\nimport javax.inject.Inject;\nimport java.util.List;\nimport com.example.Outer.Inner;\n")
	want := []string{"com.example.Outer.Inner", "com.example.Widget", "java.util.List", "javax.inject.Inject"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("imports = %v, want %v", got, want)
	}
}

func TestParseImportsStaticAndOnDemand(t *testing.T) {
	src := "package demo;\n\nimport static com.example.Widget.create;\nimport static com.example.Widget.MAX;\nimport static com.example.Tools.*;\nimport static com.example.Outer.Inner.build;\nimport static java.lang.Math.max;\nimport com.example.pkg.*;\n"
	got, _, _, _ := parse(t, src)
	want := []string{"com.example.Outer.Inner", "com.example.Tools", "com.example.Widget", "com.example.pkg", "java.lang.Math"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("imports = %v, want %v", got, want)
	}
}

func TestParseImportsCommentsAndStringsInert(t *testing.T) {
	src := "package demo;\n\n// import com.fake.Commented;\n/* import com.fake.Blocked; */\nimport java.util.List;\n\nclass Demo {\n  String s = \"import com.fake.Literal;\";\n  String b = \"\"\"\n    import com.fake.TextBlock;\n    \"\"\";\n}\n"
	got, _, _, _ := parse(t, src)
	if want := []string{"java.util.List"}; !reflect.DeepEqual(got, want) {
		t.Errorf("imports = %v, want %v", got, want)
	}
}

func TestParseImportsSpacingAndComments(t *testing.T) {
	got, _, _, _ := parse(t, "package demo;\nimport com . example /* x */ . Widget ;\nimport\n  javax.inject.Named;\n")
	if want := []string{"com.example.Widget", "javax.inject.Named"}; !reflect.DeepEqual(got, want) {
		t.Errorf("imports = %v, want %v", got, want)
	}
}

func TestParseImportsDedupedSorted(t *testing.T) {
	got, _, _, _ := parse(t, "package demo;\n\nimport com.example.Zeta;\nimport com.example.Alpha;\nimport com.example.Zeta;\n")
	if want := []string{"com.example.Alpha", "com.example.Zeta"}; !reflect.DeepEqual(got, want) {
		t.Errorf("imports = %v, want %v", got, want)
	}
}

func TestParsePackage(t *testing.T) {
	if _, got, _, _ := parse(t, "package com.example.demo;\n\npublic class Demo {}\n"); got != "com.example.demo" {
		t.Errorf("package = %q, want com.example.demo", got)
	}
	if _, got, _, _ := parse(t, "@Deprecated\npackage com.example.annotated;\n"); got != "com.example.annotated" {
		t.Errorf("annotated package = %q", got)
	}
	if _, got, _, _ := parse(t, "public class Demo {}\n"); got != "" {
		t.Errorf("default package = %q, want empty", got)
	}
}

func TestParsePackageDuplicateFails(t *testing.T) {
	if _, err := ParseSource([]byte("package a;\npackage b;\n")); err == nil || err.Error() != "java: duplicate package clause a" {
		t.Fatalf("duplicate: %v", err)
	}
}

func TestParseHeaderSyntaxErrorFails(t *testing.T) {
	_, err := ParseSource([]byte("package demo;\n\nimport com.example.;\n\nclass Demo {}\n"))
	if err == nil || !strings.HasPrefix(err.Error(), "syntax error at line 3") {
		t.Fatalf("err = %v, want a line 3 syntax error", err)
	}
}

func TestParseBodySyntaxErrorKeepsImports(t *testing.T) {
	got, _, _, _ := parse(t, "package demo;\n\nimport com.example.Widget;\n\nclass Demo { void f() { int x = ; } }\n")
	if want := []string{"com.example.Widget"}; !reflect.DeepEqual(got, want) {
		t.Errorf("imports = %v, want %v", got, want)
	}
}

func TestDefinesMain(t *testing.T) {
	if _, _, main, _ := parse(t, "package demo;\n\npublic class Main {\n  public static void main(String[] args) {}\n}\n"); !main {
		t.Error("missed a main method")
	}
	if _, _, main, _ := parse(t, "package demo;\n\npublic class Demo {\n  // public static void main(String[] args)\n  String s = \"static void main(\";\n}\n"); main {
		t.Error("matched a commented or quoted main")
	}
	if _, _, main, _ := parse(t, "package demo;\n\npublic class Demo {\n  public void main(String[] args) {}\n  public String hello() { return \"x\"; }\n}\n"); main {
		t.Error("matched an instance method")
	}
}

func TestDeclaredTypesAreQualified(t *testing.T) {
	_, _, _, got := parse(t, "package com.example;\n\npublic class Widget {}\ninterface Port {}\nenum Mode { A }\nrecord Pair(int a, int b) {}\n@interface Marker {}\n")
	want := []string{"com.example.Widget", "com.example.Port", "com.example.Mode", "com.example.Pair", "com.example.Marker"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("declared = %v, want %v", got, want)
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

func TestParseImportsStaticDeclaringType(t *testing.T) {
	src := "package demo;\n\nimport static com.example.Widget.create;\nimport static com.example.Widget.MAX;\nimport static com.example.Widget.*;\nimport static com.example.Outer.Inner.build;\nimport static java.lang.Math.max;\nimport static Local.go;\n// import static com.fake.Commented.x;\n/* import static com.fake.Blocked.y; */\nimport com.example.Gadget;\n"
	got, _, _, _ := parse(t, src)
	want := []string{"Local", "com.example.Gadget", "com.example.Outer.Inner", "com.example.Widget", "java.lang.Math"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("imports = %v, want %v", got, want)
	}
}
