package greet

import "testing"

func TestDiffEqual(t *testing.T) {
	t.Parallel()
	if got := Diff("a", "a"); got != "" {
		t.Fatalf("Diff(a,a) = %q, want empty", got)
	}
	if !Equal("a", "a") {
		t.Fatal("Equal(a,a) = false, want true")
	}
	if Equal("a", "b") {
		t.Fatal("Equal(a,b) = true, want false")
	}
}
