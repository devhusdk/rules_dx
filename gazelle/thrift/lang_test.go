package thrift

import (
	"reflect"
	"testing"
)

func TestParseImports(t *testing.T) {
	got := ParseImports([]byte("include \"b.thrift\"\n// include \"skipped.thrift\"\n# include \"hash.thrift\"\ninclude 'a/c.thrift'\nstruct S {}\n"))
	want := []string{"a/c.thrift", "b.thrift"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}
