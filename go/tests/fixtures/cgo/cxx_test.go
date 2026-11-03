package cgo

import "testing"

func TestCxxAdd(t *testing.T) {
	t.Parallel()
	if got, want := CxxAdd(2, 3), 5; got != want {
		t.Fatalf("CxxAdd(2, 3) = %d, want %d", got, want)
	}
}
