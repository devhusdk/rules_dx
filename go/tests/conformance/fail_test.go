package conformance

import "testing"

func TestFailingCase(t *testing.T) {
	t.Parallel()
	if got, want := Greet("world"), "goodbye world"; got != want {
		t.Fatalf("Greet(world) = %q, want %q", got, want)
	}
}
