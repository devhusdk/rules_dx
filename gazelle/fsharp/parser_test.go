package fsharp

import (
	"reflect"
	"testing"
)

func TestParseImportsSingleType(t *testing.T) {
	got := ParseImports([]byte("namespace Demo\n\nopen Acme.Widget\nopen System.Collections.Generic\n"))
	want := []string{"System.Collections.Generic", "Widget"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsCommentsAndStringsInert(t *testing.T) {
	got := ParseImports([]byte("namespace Demo\n\n// open Acme.Commented\n(* open Acme.Blocked *)\nopen System.Text\n\nlet s = \"open Acme.Literal\"\n"))
	want := []string{"System.Text"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsDedupedSorted(t *testing.T) {
	got := ParseImports([]byte("namespace Demo\n\nopen Acme.Zeta\nopen Acme.Alpha\nopen Acme.Zeta\n"))
	want := []string{"Alpha", "Zeta"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParseImportsMultiLineLiteralsInert(t *testing.T) {
	for _, source := range []string{
		"module Demo\n\nlet s = \"\"\"\nopen Ghost.Phantom\n\"\"\"\n",
		"module Demo\n\nlet s = $\"\"\"\nopen Ghost.Phantom\n\"\"\"\n",
		"module Demo\n\n(*\nopen Ghost.Phantom\n*)\n",
		"module Demo\n\nlet s = \"\"\"\nopen Ghost.Phantom\n",
	} {
		if got := ParseImports([]byte(source)); len(got) != 0 {
			t.Errorf("inert %q: %v", source, got)
		}
	}
}

func TestParseImportsKeepsCodeAfterMultiLineLiteral(t *testing.T) {
	got := ParseImports([]byte("module Demo\n\nlet s = \"\"\"\nopen Ghost.Phantom\n\"\"\"\nlet run () = ()\n\nopen Acme.Widget\n"))
	want := []string{"Widget"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestParsePackage(t *testing.T) {
	for _, tc := range []struct{ name, src, want string }{
		{"namespace", "namespace Acme.Demo\n\ntype Demo = class end\n", "Acme.Demo"},
		{"top module", "module Demo\n\nlet x = 1\n", GlobalPackage},
		{"dotted module", "module A.B.M\n\nlet x = 1\n", "A.B"},
		{"no declaration", "let x = 1\n", GlobalPackage},
		{"empty", "", GlobalPackage},
		{"nested module", "namespace Demo\n\nmodule Utils =\n    let x = 1\n", "Demo"},
		{"nested module first", "module Utils =\n    let x = 1\n", GlobalPackage},
		{"module rec", "module rec Acme.Demo\n", "Acme"},
		{"module internal", "module internal Acme.Demo\n", "Acme"},
		{"namespace rec", "namespace rec Acme\n", "Acme"},
		{"attribute line", "[<AutoOpen>]\nmodule Acme.Demo\n", "Acme"},
		{"inline attribute", "[<RequireQualifiedAccess>] module Acme.Demo\n", "Acme"},
		{"directive first", "#nowarn \"40\"\nnamespace Acme\n", "Acme"},
		{"comment first", "// header\n(* block *)\nnamespace Acme\n", "Acme"},
		{"two namespaces", "namespace A\ntype X = int\nnamespace B\n", "A"},
		{"namespace global", "namespace global\n\ntype X = int\n", GlobalPackage},
		{"crlf", "namespace Acme.Demo\r\n\r\ntype X = int\r\n", "Acme.Demo"},
		{"bom", "\xEF\xBB\xBFmodule Acme.Demo\n", "Acme"},
		{"open first", "open System\nlet x = 1\n", GlobalPackage},
	} {
		got, err := ParsePackage([]byte(tc.src))
		if err != nil || got != tc.want {
			t.Errorf("%s: ParsePackage = %q, %v; want %q", tc.name, got, err, tc.want)
		}
	}
	if GlobalPackage != "global" {
		t.Errorf("GlobalPackage = %q, want global", GlobalPackage)
	}
}

func TestParseDeclaredNames(t *testing.T) {
	for _, tc := range []struct {
		name, src string
		want      []string
	}{
		{"generics", "namespace Demo\n\ntype Box<'T> = { value: 'T }\ntype Pair<'a, 'b>(a: 'a, b: 'b) = class end\n", []string{"Box", "Pair"}},
		{"and", "namespace Demo\n\ntype A =\n    | Leaf\n    | Node of B\nand B = { a: A }\n", []string{"A", "B"}},
		{"let rec and", "namespace Demo\n\nmodule M =\n    let rec f x = g x\n    and g x = f x\nlet rec h x = 0\nand k x = 0\n", []string{"M"}},
		{"access modifiers", "namespace Demo\n\ntype internal Hidden = int\ntype private Secret = int\nmodule internal Utils =\n    let x = 1\n", []string{"Hidden", "Secret", "Utils"}},
		{"struct attribute", "namespace Demo\n\n[<Struct>]\ntype Point = { x: int }\ntype [<Struct>] Size = { w: int }\n", []string{"Point", "Size"}},
		{"exception", "namespace Demo\n\nexception Boom of string\n", []string{"Boom"}},
		{"nested", "namespace Demo\n\nmodule Utils =\n    type Inner = int\n    module Deep =\n        let x = 1\n", []string{"Utils"}},
		{"top module", "module Acme.Helper\n\ntype Inner = int\nmodule Sub =\n    let x = 1\n", []string{"Helper"}},
		{"implicit module", "type Loose = int\n", nil},
		{"commented", "namespace Demo\n\n// type Ghost = int\n(* type Phantom = int *)\nlet s = \"\"\"\ntype Literal = int\n\"\"\"\n", nil},
	} {
		got := ParseDeclaredNames([]byte(tc.src))
		if len(got) == 0 && len(tc.want) == 0 {
			continue
		}
		if !reflect.DeepEqual(got, tc.want) {
			t.Errorf("%s: ParseDeclaredNames = %v, want %v", tc.name, got, tc.want)
		}
	}
}

func TestParseReferencedNames(t *testing.T) {
	for _, tc := range []struct {
		name, src, pkg string
		want           []string
	}{
		{"dotted head", "namespace Demo\n\nlet x = Helper.suffix + Acme.Widget.Run()\n", "Demo", []string{"Acme", "Helper", "let", "x"}},
		{"own namespace stripped", "namespace Acme.Demo\n\nlet x = Acme.Demo.Helper.suffix\n", "Acme.Demo", []string{"Helper", "let", "x"}},
		{"global keeps head", "module Demo\n\nlet x = Helper.suffix\n", GlobalPackage, []string{"Helper", "let", "x"}},
		{"member access skipped", "namespace Demo\n\nlet y = (f ()).Name\n", "Demo", []string{"f", "let", "y"}},
		{"unqualified type", "namespace Demo\n\ntype A = { b: B }\n", "Demo", []string{"A", "B", "b", "type"}},
		{"comments and strings", "namespace Demo\n\n// Ghost.x\n(* Phantom.y *)\nlet s = \"Literal.z\"\nlet t = \"\"\"\nTriple.w\n\"\"\"\n", "Demo", []string{"let", "s", "t"}},
		{"type variables", "namespace Demo\n\nlet f (x: 'Helper) = x\nlet x' = 1\n", "Demo", []string{"f", "let", "x", "x'"}},
		{"header lines skipped", "namespace Demo\n\nopen Helper\nopen Acme.Widget\n", "Demo", nil},
		{"numbers", "namespace Demo\n\nlet n = 1e5 + 0x1F + 2.0\n", "Demo", []string{"let", "n"}},
	} {
		got := ParseReferencedNames([]byte(tc.src), tc.pkg)
		if len(got) == 0 && len(tc.want) == 0 {
			continue
		}
		if !reflect.DeepEqual(got, tc.want) {
			t.Errorf("%s: ParseReferencedNames = %v, want %v", tc.name, got, tc.want)
		}
	}
}

func TestDefinesMain(t *testing.T) {
	if !DefinesMain([]byte("module Main\n\n[<EntryPoint>]\nlet main _ = 0\n")) {
		t.Error("DefinesMain missed an entry point")
	}
	if DefinesMain([]byte("module Demo\n\n// [<EntryPoint>]\nlet x = 1\n")) {
		t.Error("DefinesMain matched a commented entry point")
	}
	if DefinesMain([]byte("module Demo\n\nlet x = 1\n")) {
		t.Error("DefinesMain matched a library source")
	}
}

func TestMultiplyOperatorIsNotComment(t *testing.T) {
	src := []byte("module App\nlet product xs = List.fold (*) 1 xs\nopen Acme.Core\n[<EntryPoint>]\nlet main _ = 0\n")
	got := ParseImports(src)
	if !reflect.DeepEqual(got, []string{"Core"}) {
		t.Errorf("ParseImports = %v, want [Core]", got)
	}
	if !DefinesMain(src) {
		t.Error("DefinesMain missed an entry point after (*)")
	}
}

func TestIsStdLib(t *testing.T) {
	for _, imp := range []string{"System.Collections.Generic", "System.Text", "System", "System.IO", "FSharp.Collections", "Microsoft.FSharp.Core"} {
		if !IsStdLib(imp) {
			t.Errorf("IsStdLib(%q) = false, want true", imp)
		}
	}
	for _, imp := range []string{"Acme.Widget", "Widget", "Microsoft.Extensions.Logging", "System.CommandLine", "FSharp.Compiler.Text", "FSharp.Data.Runtime"} {
		if IsStdLib(imp) {
			t.Errorf("IsStdLib(%q) = true, want false", imp)
		}
	}
}

func TestLiteralAndTruncatedSourceRemainInert(t *testing.T) {
	for _, source := range []string{
		"/*\nopen Fake.Block\n*/\n", "/*\nopen Fake.Unterminated", "\"escaped\\\" open Fake.String\"\n", "\"unterminated\n",
		"'x'", `'\''`, "'unterminated", "'unterminated\n",
	} {
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
}

func TestParseImportsTypeVariablesAndPrimes(t *testing.T) {
	source := "namespace Demo\n\nlet f (x: 'a) = x\nlet g<'T> (y: 'T) = y\nlet x' = 1\nlet z'' = 2\nlet c = '\"'\nopen Acme.Widget\n"
	got := ParseImports([]byte(source))
	want := []string{"Widget"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestDefinesMainAfterTypeVariables(t *testing.T) {
	source := "module Program\n\nlet f (x: 'a) = x\nlet x' = 1\n\n[<EntryPoint>]\nlet main argv = 0\n"
	if !DefinesMain([]byte(source)) {
		t.Errorf("DefinesMain = false for %q", source)
	}
}
