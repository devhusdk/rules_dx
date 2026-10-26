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
	got, err := ParsePackage([]byte("namespace Acme.Demo\n\ntype Demo = class end\n"))
	if err != nil || got != "Acme.Demo" {
		t.Errorf("ParsePackage = %q, %v; want Acme.Demo", got, err)
	}
	if got, err := ParsePackage([]byte("module Demo\n\nlet x = 1\n")); err != nil || got != "Demo" {
		t.Errorf("ParsePackage module = %q, %v; want Demo", got, err)
	}
	if got, err := ParsePackage([]byte("let x = 1\n")); err != nil || got != "" {
		t.Errorf("ParsePackage default = %q, %v; want empty", got, err)
	}
	if _, err := ParsePackage([]byte("namespace A\nnamespace B\n")); err == nil {
		t.Fatal("ParsePackage succeeded on duplicate declarations")
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
	if _, err := ParsePackage([]byte("namespace A\nnamespace B\n")); err == nil || err.Error() != "fsharp: duplicate namespace/module declaration A" {
		t.Fatalf("duplicate: %v", err)
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
