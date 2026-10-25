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
	src := "package demo\n\nimport cats.effect.{IO, IOApp}\nimport zio._\nimport com.example.tools.*\nimport a.b.{C => D, E as F, _}\nimport q.R as S\nimport x.y.Z, u.v.W\nimport _root_.org.lib.Thing\nimport p.q.{X, *}\n"
	got, _, _, _ := parse(t, src)
	want := []string{"a.b", "a.b.C", "a.b.E", "cats.effect.IO", "cats.effect.IOApp", "com.example.tools", "org.lib.Thing", "p.q", "p.q.X", "q.R", "u.v.W", "x.y.Z", "zio"}
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

func TestParseImportsBraceSelectors(t *testing.T) {
	got, _, _, _ := parse(t, "package demo\n\nimport com.example.{Widget, Gadget}\nimport com.other.{Tool}; import com.more.Thing\n")
	want := []string{"com.example.Gadget", "com.example.Widget", "com.more.Thing", "com.other.Tool"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("imports = %v, want %v", got, want)
	}
}

func TestParseImportsUnderscoreWildcard(t *testing.T) {
	got, _, _, _ := parse(t, "package demo\n\nimport a.b._\nimport c.D.{_}\n")
	want := []string{"a.b", "c.D"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("imports = %v, want %v", got, want)
	}
}

func TestParseImportsScala3StarAndGiven(t *testing.T) {
	got, _, _, _ := parse(t, "package demo\n\nimport a.B.*\nimport a.C.given\nimport a.D.{given, *}\nimport a.E.{given Ord[Int, String]}\nimport a.F.given Ord[Int]\n")
	want := []string{"a.B", "a.C", "a.D", "a.E", "a.F"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("imports = %v, want %v", got, want)
	}
}

func TestParseImportsRenameAndHide(t *testing.T) {
	got, _, _, _ := parse(t, "package demo\n\nimport a.{A => B}\nimport a.{C as D}\nimport b.{E => _, _}\nimport c.{F => _}\nimport d.{G as _}\n")
	want := []string{"a.A", "a.C", "b"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("imports = %v, want %v", got, want)
	}
}

func TestParseImportsMultilineBraces(t *testing.T) {
	got, _, _, _ := parse(t, "package demo\n\nimport com.example.{\n  Widget,\n  // Commented,\n  Gadget => G\n}\nimport com.example.Tail\n")
	want := []string{"com.example.Gadget", "com.example.Tail", "com.example.Widget"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("imports = %v, want %v", got, want)
	}
}

func TestParseImportsStdlibSelectors(t *testing.T) {
	got, _, _, _ := parse(t, "package demo\n\nimport scala.collection.mutable.{Map => MMap, Set}\nimport java.util.{List, Map => JMap}\n")
	want := []string{"java.util.List", "java.util.Map", "scala.collection.mutable.Map", "scala.collection.mutable.Set"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("imports = %v, want %v", got, want)
	}
}

func TestParseImportsCommaImporters(t *testing.T) {
	got, _, _, _ := parse(t, "package demo\n\nimport a.B, c.d._, e.{F, G}\n")
	want := []string{"a.B", "c.d", "e.F", "e.G"}
	if !reflect.DeepEqual(got, want) {
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

func TestParseStrayHeaderStatementFails(t *testing.T) {
	_, err := ParseSource([]byte("package demo\n\nfoo bar\n\nclass Demo\n"))
	if err == nil || err.Error() != "syntax error at line 3 column 1" {
		t.Fatalf("err = %v, want line 3 column 1", err)
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
	for _, imp := range []string{"scala.collection.immutable.List", "scala.collection.mutable.Map", "scala.reflect.ClassTag", "scala.reflect.runtime.universe", "scala.jdk.CollectionConverters", "scala.math.max", "scala.annotation.tailrec", "scala.language.implicitConversions", "scala.language.experimental.macros", "scala.util.chaining.*", "scala.collection.mutable.*", "scala.concurrent.duration.DurationInt", "scala.reflect.runtime.universe.*", "scala.concurrent.ExecutionContext.Implicits.global", "java.util.*", "java.util.List", "java.util", "javax.swing.JFrame", "javax.xml.parsers.DocumentBuilder", "jdk.internal.Foo", "org.w3c.dom.Node", "org.xml.sax.Parser"} {
		if !IsStdLib(imp) {
			t.Errorf("IsStdLib(%q) = false, want true", imp)
		}
	}
	for _, imp := range []string{"com.example.Widget", "Widget", "javax.inject.Inject", "javax.servlet.Servlet", "javax.annotation.Nullable", "javax.xml.bind.JAXBContext", "javafx.application.Application", "javaxfoo.Bar", "com.sun.jna.Native", "sun.foo.Bar", "scala.xml.Elem", "scala.util.parsing.combinator.RegexParsers", "scala.collection.parallel.CollectionConverters", "scala.swing.Frame", "scala.async.Async", "scala.tools.nsc.Global", "scalaz.Monad", "scala.collection.compat.*", "scala.collection.compat.immutable.ArraySeq", "scala.meta.*", "scala.meta.Term", "scala.compat.java8.FutureConverters", "scala.collection.decorators.*", "scala.xml.*", "scala.util.parsing.*"} {
		if IsStdLib(imp) {
			t.Errorf("IsStdLib(%q) = true, want false", imp)
		}
	}
}
