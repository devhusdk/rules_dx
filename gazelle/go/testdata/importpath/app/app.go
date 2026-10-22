package app

import (
	"github.com/google/go-cmp/cmp"

	alib "example.test/demo/a/lib"
	"example.test/demo/lib"
)

func Run() string {
	return cmp.Diff(alib.A(), lib.Name())
}
