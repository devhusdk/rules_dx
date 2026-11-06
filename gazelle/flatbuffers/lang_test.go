package flatbuffers

import (
	"reflect"
	"testing"
)

func TestParseImports(t *testing.T) {
	got := ParseImports([]byte("include \"b.fbs\";\n// include \"skipped.fbs\";\ninclude \"a/c.fbs\";\ntable T {}\n"))
	want := []string{"a/c.fbs", "b.fbs"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}
