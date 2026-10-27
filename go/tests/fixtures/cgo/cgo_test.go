package cgo

import "testing"

func TestHello(t *testing.T) {
	t.Parallel()
	if got, want := Hello("world"), "hello world"; got != want {
		t.Fatalf("Hello(world) = %q, want %q", got, want)
	}
}

func TestAdd(t *testing.T) {
	t.Parallel()
	if got, want := Add(2, 3), 5; got != want {
		t.Fatalf("Add(2, 3) = %d, want %d", got, want)
	}
}
