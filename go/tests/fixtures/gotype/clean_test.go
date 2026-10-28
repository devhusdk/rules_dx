package clean_test

import (
	"testing"

	"rules_dx/go/tests/fixtures/gotype/clean"
)

func TestGreeting(t *testing.T) {
	if clean.Greeting() == "" {
		t.Fatal("empty greeting")
	}
}
