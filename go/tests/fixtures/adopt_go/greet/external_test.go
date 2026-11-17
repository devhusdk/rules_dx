package greet_test

import "testing"

func TestExternalPlaceholder(t *testing.T) {
	t.Parallel()
	if "hello world" != "hello world" {
		t.Fatal("unreachable")
	}
}
