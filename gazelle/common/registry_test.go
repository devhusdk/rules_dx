package common

import (
	"testing"

	"github.com/bazelbuild/bazel-gazelle/language"
)

type namedLang struct {
	language.BaseLang
	name string
}

func (l *namedLang) Name() string { return l.name }

func withRegistry(t *testing.T, names ...string) {
	t.Helper()
	saved := registry
	registry = nil
	t.Cleanup(func() { registry = saved })
	for _, name := range names {
		RegisterLanguage(func() language.Language { return &namedLang{name: name} })
	}
}

func TestRegisteredLanguagesSortedByName(t *testing.T) {
	withRegistry(t, "zig", "astro", "go")
	var got []string
	for _, l := range RegisteredLanguages() {
		got = append(got, l.Name())
	}
	if want := "astro go zig"; len(got) != 3 || got[0]+" "+got[1]+" "+got[2] != want {
		t.Fatalf("names = %v, want %s", got, want)
	}
}

func TestRegisteredLanguagesPanicsOnDuplicate(t *testing.T) {
	withRegistry(t, "go", "go")
	defer func() {
		if recover() == nil {
			t.Fatal("expected panic for duplicate name")
		}
	}()
	RegisteredLanguages()
}
