package scala

import (
	"reflect"
	"testing"
)

func TestParseImportsSingleType(t *testing.T) {
	got := ParseImports([]byte("package demo\n\nimport com.example.Widget\nimport java.util.List\n"))
	want := []string{"com.example.Widget", "java.util.List"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsAliasAndOnDemand(t *testing.T) {
	got := ParseImports([]byte("package demo\n\nimport com.example.Widget as W\nimport com.example.*\n"))
	want := []string{"com.example.*", "com.example.Widget"}
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
	want := []string{"com.example.Alpha", "com.example.Zeta"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsBraceSelectors(t *testing.T) {
	got := ParseImports([]byte("package demo\n\nimport com.example.{Widget, Gadget}\nimport com.other.{Tool}; import com.more.Thing\n"))
	want := []string{"com.example.Gadget", "com.example.Widget", "com.more.Thing", "com.other.Tool"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsUnderscoreWildcard(t *testing.T) {
	got := ParseImports([]byte("package demo\n\nimport a.b._\nimport c.D.{_}\n"))
	want := []string{"a.b.*", "c.D.*"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsScala3StarAndGiven(t *testing.T) {
	got := ParseImports([]byte("package demo\n\nimport a.B.*\nimport a.C.given\nimport a.D.{given, *}\nimport a.E.{given Ord[Int, String]}\nimport a.F.given Ord[Int]\n"))
	want := []string{"a.B.*", "a.C.*", "a.D.*", "a.E.*", "a.F.*"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsRenameAndHide(t *testing.T) {
	got := ParseImports([]byte("package demo\n\nimport a.{A => B}\nimport a.{C as D}\nimport b.{E => _, _}\nimport c.{F => _}\n"))
	want := []string{"a.A", "a.C", "b.*"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsMultilineBraces(t *testing.T) {
	got := ParseImports([]byte("package demo\n\nimport com.example.{\n  Widget,\n  // Commented,\n  Gadget => G\n}\nimport com.example.Tail\n"))
	want := []string{"com.example.Gadget", "com.example.Tail", "com.example.Widget"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsStdlibSelectors(t *testing.T) {
	got := ParseImports([]byte("package demo\n\nimport scala.collection.mutable.{Map => MMap, Set}\nimport java.util.{List, Map => JMap}\n"))
	want := []string{"java.util.List", "java.util.Map", "scala.collection.mutable.Map", "scala.collection.mutable.Set"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsCommaImporters(t *testing.T) {
	got := ParseImports([]byte("package demo\n\nimport a.B, c.d._, e.{F, G}\n"))
	want := []string{"a.B", "c.d.*", "e.F", "e.G"}
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
}

func TestDefinesMain(t *testing.T) {
	if !DefinesMain([]byte("package demo\n\ndef main() {}\n")) {
		t.Error("DefinesMain missed a main function")
	}
	if !DefinesMain([]byte("package demo\n\ndef main(args: Array[String]) {}\n")) {
		t.Error("DefinesMain missed a main function with args")
	}
	if DefinesMain([]byte("package demo\n\nclass Demo {\n  // def main()\n}\n")) {
		t.Error("DefinesMain matched a commented main")
	}
	if DefinesMain([]byte("package demo\n\nclass Demo {\n  def hello(): String { return \"x\" }\n}\n")) {
		t.Error("DefinesMain matched a library source")
	}
}

func TestDefinesMainScala3(t *testing.T) {
	for _, src := range []string{"@main def run() = ()", "@main\ndef run() = ()", "object App extends App { }", "object M extends scala.App", "object M extends _root_.scala.App"} {
		if !DefinesMain([]byte(src)) {
			t.Errorf("DefinesMain(%q) = false, want true", src)
		}
	}
	for _, src := range []string{"class A extends AppConfig", "@mainly def run() = ()", "// @main def run()\nclass A", "val s = \"extends App\"", "/* @main def x */ class A"} {
		if DefinesMain([]byte(src)) {
			t.Errorf("DefinesMain(%q) = true, want false", src)
		}
	}
}

func TestIsStdLib(t *testing.T) {
	for _, imp := range []string{"scala.collection.immutable.List", "scala.collection.mutable.Map", "scala.concurrent.Future", "scala.math.max", "scala.Option", "scala.collection", "java.util.List", "java.util", "javax.swing.JFrame", "javax.xml.parsers.DocumentBuilder", "jdk.internal.Foo", "org.w3c.dom.Node", "org.xml.sax.Parser", "scala.language.implicitConversions", "scala.language.postfixOps", "scala.language.higherKinds", "scala.language.experimental.macros", "scala.util.chaining.scalaUtilChainingOps", "scala.Predef.ArrowAssoc", "scala.Predef.augmentString"} {
		if !IsStdLib(imp) {
			t.Errorf("IsStdLib(%q) = false, want true", imp)
		}
	}
	for _, imp := range []string{"com.example.Widget", "Widget", "javax.inject.Inject", "javax.servlet.Servlet", "javax.annotation.Nullable", "javax.xml.bind.JAXBContext", "javafx.application.Application", "javaxfoo.Bar", "com.sun.jna.Native", "sun.foo.Bar", "scala.xml.Elem", "scala.xml", "scala.util.parsing.combinator.Parsers", "scala.util.parsing", "scala.collection.parallel.CollectionConverters", "scala.collection.parallel", "scala.reflect.runtime.universe", "scala.reflect.runtime", "org.scalatest.funsuite.AnyFunSuite"} {
		if IsStdLib(imp) {
			t.Errorf("IsStdLib(%q) = true, want false", imp)
		}
	}
}

func TestScalaFeatureImportsAreStdLib(t *testing.T) {
	src := "package demo\nimport scala.language.implicitConversions\nimport scala.language.{higherKinds, postfixOps}\nimport scala.util.chaining.scalaUtilChainingOps\nimport scala.util.chaining._\n"
	for _, imp := range ParseImports([]byte(src)) {
		if !IsStdLib(imp) {
			t.Errorf("IsStdLib(%q) = false, want true", imp)
		}
	}
}

func TestParsePackageChained(t *testing.T) {
	for _, src := range []string{"package com.example\npackage demo\n", "package com.example;\npackage demo;\n"} {
		if got, err := ParsePackage([]byte(src)); err != nil || got != "com.example.demo" {
			t.Errorf("ParsePackage(%q) = %q, %v; want com.example.demo", src, got, err)
		}
	}
}

func TestParsePackageBlock(t *testing.T) {
	if got, err := ParsePackage([]byte("package demo {\n  class X\n}\n")); err != nil || got != "demo" {
		t.Errorf("block = %q, %v; want demo", got, err)
	}
	if got, err := ParsePackage([]byte("package a {\n  package b {\n  }\n}\n")); err != nil || got != "a.b" {
		t.Errorf("nested = %q, %v; want a.b", got, err)
	}
}

func TestParsePackageSeparateFails(t *testing.T) {
	for _, src := range []string{"package a {}\npackage b {}\n", "package a\nclass X\npackage b\n", "package a { class X }\npackage b\n"} {
		if _, err := ParsePackage([]byte(src)); err == nil || err.Error() != "scala: separate package clauses a, b" {
			t.Errorf("ParsePackage(%q) error = %v", src, err)
		}
	}
}

func TestParsePackageObjectIgnored(t *testing.T) {
	if got, err := ParsePackage([]byte("package demo\npackage object util {\n}\n")); err != nil || got != "demo" {
		t.Errorf("ParsePackage = %q, %v; want demo", got, err)
	}
}

func TestNestedBlockCommentInert(t *testing.T) {
	if got := ParseImports([]byte("/* outer /* inner */\nimport com.acme.Secret\n*/\npackage app\n")); len(got) != 0 {
		t.Errorf("ParseImports = %v, want none", got)
	}
	if DefinesMain([]byte("/* a /* b */\ndef main() = 0\n*/\n")) {
		t.Error("DefinesMain matched a main inside a nested comment")
	}
}

// Intended behavior change from the regex parser: clauses sharing one line
// (package a; package b) are chained.
func TestParsePackageSameLine(t *testing.T) {
	if got, err := ParsePackage([]byte("package a; package b\nclass X\n")); err != nil || got != "a.b" {
		t.Errorf("ParsePackage = %q, %v; want a.b", got, err)
	}
}

func TestParseImportsTreeSitterShapes(t *testing.T) {
	tests := []struct {
		name, src string
		want      []string
	}{
		{"interpolated string inert", "import a.B\nobject O { val s = s\"import x.Y ${1}\" }\n", []string{"a.B"}},
		{"block local import", "import a.B\nobject O { def f() = { import loc.L; 1 } }\n", []string{"a.B", "loc.L"}},
		{"truncated import", "import a.B\nimport c.D\nimport e.", []string{"a.B", "c.D"}},
		{"unclosed string after imports", "package p\nimport a.B\nimport c.D\nobject O { val s = \"oops\n", []string{"a.B", "c.D"}},
		{"given selector", "import x.{given Foo, Y}\n", []string{"x.*", "x.Y"}},
		{"hide with as", "import x.{Y as _, Z as W}\n", []string{"x.Z"}},
		{"comment between segments", "import a./*c*/B\n", []string{"a.B"}},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := ParseImports([]byte(tt.src)); !reflect.DeepEqual(got, tt.want) {
				t.Errorf("ParseImports = %v, want %v", got, tt.want)
			}
		})
	}
}
