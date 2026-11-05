package protobuf

import (
	"reflect"
	"testing"
)

func TestParseImports(t *testing.T) {
	src := `syntax = "proto3";
package demo;

import "pkg/x.proto";
import public "google/protobuf/timestamp.proto";
import weak 'other/y.proto';
// import "commented/out.proto";
/* import "block/out.proto"; */
import "pkg/x.proto";
message M { string s = 1; }
`
	want := []string{"google/protobuf/timestamp.proto", "other/y.proto", "pkg/x.proto"}
	if got := ParseImports([]byte(src)); !reflect.DeepEqual(got, want) {
		t.Errorf("ParseImports = %v, want %v", got, want)
	}
}

func TestHasService(t *testing.T) {
	cases := map[string]bool{
		"service Greeter {\n  rpc Hi(A) returns (B);\n}": true,
		"  service Greeter{}":                               true,
		"// service Greeter {}":                             false,
		"/* service Greeter {} */":                          false,
		"message Service { string service = 1; }":           false,
	}
	for in, want := range cases {
		if got := HasService([]byte(in)); got != want {
			t.Errorf("HasService(%q) = %v, want %v", in, got, want)
		}
	}
}
