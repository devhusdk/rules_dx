package avro

import (
	"reflect"
	"testing"
)

func TestParseImports(t *testing.T) {
	got := ParseImports([]byte("protocol P {\n  import idl \"b.avdl\";\n  import schema \"a/c.avsc\";\n  import protocol \"skip.avpr\";\n  // import idl \"skipped.avdl\";\n}\n"))
	want := []string{"a/c.avsc", "b.avdl"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}
