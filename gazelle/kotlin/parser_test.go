package kotlin

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
	got, _, _, _ := parse(t, "package demo\n\nimport com.example.Widget\nimport kotlinx.coroutines.runBlocking\nimport java.util.List\n")
	want := []string{"com.example.Widget", "java.util.List", "kotlinx.coroutines.runBlocking"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("imports = %v, want %v", got, want)
	}
}

func TestParseImportsAliasAndOnDemand(t *testing.T) {
	got, _, _, _ := parse(t, "package demo\n\nimport com.example.Widget as W\nimport com.example.tools.*\nimport `com`.example.`Quoted`\n")
	want := []string{"com.example.Quoted", "com.example.Widget", "com.example.tools"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("imports = %v, want %v", got, want)
	}
}

func TestParseImportsCommentsAndStringsInert(t *testing.T) {
	src := "package demo\n\n// import com.fake.Commented\n/* import com.fake.Blocked */\nimport java.util.List\n\nval s = \"import com.fake.Literal\"\nval r = \"\"\"\nimport com.fake.Raw\n\"\"\"\n"
	got, _, _, _ := parse(t, src)
	if want := []string{"java.util.List"}; !reflect.DeepEqual(got, want) {
		t.Errorf("imports = %v, want %v", got, want)
	}
}

func TestParseImportsDedupedSorted(t *testing.T) {
	got, _, _, _ := parse(t, "package demo\n\nimport com.example.Zeta\nimport com.example.Alpha\nimport com.example.Zeta\n")
	if want := []string{"com.example.Alpha", "com.example.Zeta"}; !reflect.DeepEqual(got, want) {
		t.Errorf("imports = %v, want %v", got, want)
	}
}

func TestParsePackage(t *testing.T) {
	if _, got, _, _ := parse(t, "package com.example.demo\n\nclass Demo\n"); got != "com.example.demo" {
		t.Errorf("package = %q, want com.example.demo", got)
	}
	if _, got, _, _ := parse(t, "@file:JvmName(\"Tools\")\npackage com.example.tools\n\nfun tool() = 1\n"); got != "com.example.tools" {
		t.Errorf("annotated package = %q", got)
	}
	if _, got, _, _ := parse(t, "class Demo\n"); got != "" {
		t.Errorf("default package = %q, want empty", got)
	}
}

func TestParseDuplicatePackageFails(t *testing.T) {
	_, err := ParseSource([]byte("package a\npackage b\nimport x.Y\n"))
	if err == nil || !strings.HasPrefix(err.Error(), "syntax error at line 2") {
		t.Fatalf("err = %v, want a line 2 syntax error", err)
	}
}

func TestParseHeaderSyntaxErrorFails(t *testing.T) {
	_, err := ParseSource([]byte("package demo\n\nimport com.example.\n\nclass Demo\n"))
	if err == nil || !strings.HasPrefix(err.Error(), "syntax error at line 3") {
		t.Fatalf("err = %v, want a line 3 syntax error", err)
	}
}

func TestDefinesMain(t *testing.T) {
	for _, src := range []string{"package demo\n\nfun main() {}\n", "package demo\n\nfun main(args: Array<String>) {}\n"} {
		if _, _, main, _ := parse(t, src); !main {
			t.Errorf("missed main in %q", src)
		}
	}
	for _, src := range []string{"package demo\n\nclass Demo {\n  // fun main()\n  val s = \"fun main(\"\n}\n", "package demo\n\nclass Demo {\n  fun hello(): String { return \"x\" }\n}\n"} {
		if _, _, main, _ := parse(t, src); main {
			t.Errorf("matched main in %q", src)
		}
	}
}

func TestDeclaredNamesAreQualified(t *testing.T) {
	_, _, _, got := parse(t, "package com.example\n\nclass Widget\nobject Registry\ninterface Port\nfun helper() = 1\nfun String.shout() = this\nval limit: Int = 3\ntypealias Name = String\n")
	want := []string{"com.example.Widget", "com.example.Registry", "com.example.Port", "com.example.helper", "com.example.shout", "com.example.limit", "com.example.Name"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("declared = %v, want %v", got, want)
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
