package scala

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
	got, _, _, _ := parse(t, "package demo\n\nimport com.example.Widget\nimport cats.effect.IO\nimport java.util.List\n")
	want := []string{"cats.effect.IO", "com.example.Widget", "java.util.List"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("imports = %v, want %v", got, want)
	}
}

func TestParseImportsSelectorsWildcardsAndRenames(t *testing.T) {
	src := "package demo\n\nimport cats.effect.{IO, IOApp}\nimport zio._\nimport com.example.tools.*\nimport a.b.{C => D, E as F, _}\nimport q.R as S\nimport x.y.Z, u.v.W\nimport _root_.org.lib.Thing\nimport given.{given, *}\n"
	got, _, _, _ := parse(t, src)
	want := []string{"a.b", "a.b.C", "a.b.E", "cats.effect.IO", "cats.effect.IOApp", "com.example.tools", "given", "org.lib.Thing", "q.R", "u.v.W", "x.y.Z", "zio"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("imports = %v, want %v", got, want)
	}
}

func TestParseImportsInsideBodies(t *testing.T) {
	got, _, _, _ := parse(t, "package demo\n\nobject Demo {\n  import com.example.Local\n  def f(): Int = { import com.example.Deep; 1 }\n}\n")
	if want := []string{"com.example.Deep", "com.example.Local"}; !reflect.DeepEqual(got, want) {
		t.Errorf("imports = %v, want %v", got, want)
	}
}

func TestParseImportsCommentsAndStringsInert(t *testing.T) {
	src := "package demo\n\n// import com.fake.Commented\n/* import com.fake.Blocked */\nimport java.util.List\n\nobject Demo {\n  val s = \"import com.fake.Literal\"\n  val r = \"\"\"\nimport com.fake.Raw\n\"\"\"\n}\n"
	got, _, _, _ := parse(t, src)
	if want := []string{"java.util.List"}; !reflect.DeepEqual(got, want) {
		t.Errorf("imports = %v, want %v", got, want)
	}
}

func TestParsePackage(t *testing.T) {
	cases := map[string]string{
		"package com.example.demo\n\nclass Demo\n":           "com.example.demo",
		"package com.example\npackage demo\n\nclass Demo\n":   "com.example.demo",
		"package com.example {\n  package demo {\n    class Demo\n  }\n}\n": "com.example.demo",
		"class Demo\n": "",
	}
	for src, want := range cases {
		if _, got, _, _ := parse(t, src); got != want {
			t.Errorf("package of %q = %q, want %q", src, got, want)
		}
	}
}

func TestParseSiblingPackagingsFail(t *testing.T) {
	_, err := ParseSource([]byte("package a {\n  class A\n}\npackage b {\n  class B\n}\n"))
	if err == nil || err.Error() != "scala: duplicate package clause a" {
		t.Fatalf("err = %v", err)
	}
}

func TestParseHeaderSyntaxErrorFails(t *testing.T) {
	_, err := ParseSource([]byte("package demo\n\nimport com.example.{\n\nclass Demo\n"))
	if err == nil || !strings.HasPrefix(err.Error(), "syntax error at line") {
		t.Fatalf("err = %v, want a syntax error", err)
	}
}

func TestDefinesMain(t *testing.T) {
	for _, src := range []string{"package demo\n\ndef main() = ()\n", "package demo\n\nobject Main {\n  def main(args: Array[String]): Unit = ()\n}\n"} {
		if _, _, main, _ := parse(t, src); !main {
			t.Errorf("missed main in %q", src)
		}
	}
	for _, src := range []string{"package demo\n\nclass Demo {\n  // def main()\n  val s = \"def main(\"\n}\n", "package demo\n\nclass Demo {\n  def hello(): String = \"x\"\n}\n"} {
		if _, _, main, _ := parse(t, src); main {
			t.Errorf("matched main in %q", src)
		}
	}
}

func TestDeclaredNamesAreQualified(t *testing.T) {
	_, _, _, got := parse(t, "package com.example\n\nclass Widget\nobject Registry\ntrait Port\ncase class Pair(a: Int)\ndef helper(): Int = 1\nval limit = 3\ntype Name = String\n")
	want := []string{"com.example.Widget", "com.example.Registry", "com.example.Port", "com.example.Pair", "com.example.helper", "com.example.limit", "com.example.Name"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("declared = %v, want %v", got, want)
	}
}

func TestIsStdLib(t *testing.T) {
	for _, imp := range []string{"scala.collection.immutable.List", "java.util.List", "java.util", "javax.swing.JFrame", "javax.xml.parsers.DocumentBuilder", "jdk.internal.Foo", "org.w3c.dom.Node", "org.xml.sax.Parser"} {
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
