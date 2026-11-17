package greet

import (
	"os"
	"runtime"
	"testing"
)

func TestMain(m *testing.M) {
	os.Exit(m.Run())
}

func TestHello(t *testing.T) {
	t.Parallel()
	if got, want := Hello("world"), "hello world"; got != want {
		t.Fatalf("Hello(world) = %q, want %q", got, want)
	}
	want := "other"
	if runtime.GOOS == "linux" {
		want = "linux"
	}
	if got := Platform(); got != want {
		t.Fatalf("Platform() = %q, want %q (GOOS=%s)", got, want, runtime.GOOS)
	}
}

func TestShout(t *testing.T) {
	t.Parallel()
	if got, want := Shout("hi"), "HI!"; got != want {
		t.Fatalf("Shout(hi) = %q, want %q", got, want)
	}
	if got := greetHelper("ab"); got != "ab-ab" {
		t.Fatalf("greetHelper(ab) = %q, want %q", got, "ab-ab")
	}
}
