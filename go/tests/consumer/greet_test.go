package consumer

import "testing"

func TestGreet(t *testing.T) {
	t.Parallel()
	if got := Greet("world"); got != "hello world" {
		t.Fatalf("Greet = %q", got)
	}
}
