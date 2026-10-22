package app

import (
	"testing"

	blib "example.test/demo/b/lib"
)

func TestRun(t *testing.T) {
	t.Parallel()
	if Run() == "" || blib.B() != "b" {
		t.Fatal("unexpected")
	}
}
