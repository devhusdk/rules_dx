package capnproto

import (
	"reflect"
	"testing"
)

func TestParseImports(t *testing.T) {
	got := ParseImports([]byte("@0xdbb9ad1f14bf0b36;\nusing B = import \"b.capnp\";\n# using C = import \"skipped.capnp\";\nusing import \"/a/c.capnp\".D;\n"))
	want := []string{"/a/c.capnp", "b.capnp"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}
