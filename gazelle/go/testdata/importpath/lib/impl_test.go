package lib_test

import (
	"testing"

	"example.test/demo/lib"
)

func TestUpper(t *testing.T) {
	t.Parallel()
	if lib.Upper() != "LIB" {
		t.Fatal(lib.Upper())
	}
}
