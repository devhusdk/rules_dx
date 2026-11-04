package greet

import "github.com/google/go-cmp/cmp"

func Diff(x, y string) string {
	return cmp.Diff(x, y)
}

func Equal(x, y string) bool {
	return cmp.Diff(x, y) == ""
}
