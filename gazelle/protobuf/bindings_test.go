package protobuf

import "testing"

func TestGoImportPath(t *testing.T) {
	cases := map[string]string{
		`option go_package = "example.com/a;apb";`: "example.com/a",
		`option go_package = 'example.com/b';`:     "example.com/b",
		`// option go_package = "x";`:             "",
		`syntax = "proto3";`:                      "",
	}
	for in, want := range cases {
		if got := GoImportPath([]byte(in)); got != want {
			t.Errorf("GoImportPath(%q) = %q, want %q", in, got, want)
		}
	}
}
