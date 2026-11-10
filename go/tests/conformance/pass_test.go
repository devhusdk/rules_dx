package conformance

import "testing"

func TestPassingCase(t *testing.T) {
	t.Parallel()
	if got, want := Greet("world"), "hello world"; got != want {
		t.Fatalf("Greet(world) = %q, want %q", got, want)
	}
}
