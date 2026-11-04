package swift

import (
	"reflect"
	"testing"
)

func TestParseImports(t *testing.T) {
	src := `// import Commented
/* import Block */
import Foundation
@testable import Greeter
public import Util
import struct Other.Thing
let s = "x"
`
	got := ParseImports([]byte(src))
	want := []string{"Foundation", "Greeter", "Other", "Util"}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("ParseImports = %v, want %v", got, want)
	}
}

func TestDefinesMain(t *testing.T) {
	if !DefinesMain([]byte("@main\nstruct App {}\n")) {
		t.Error("@main not detected")
	}
	if DefinesMain([]byte("// @main\nstruct App {}\n")) {
		t.Error("commented @main detected")
	}
}

func TestIsStdLib(t *testing.T) {
	if !IsStdLib("Foundation") || !IsStdLib("XCTest") || IsStdLib("Greeter") {
		t.Error("IsStdLib classifies wrongly")
	}
}
