package dispatch

import (
	"slices"
	"testing"
)

func TestGazelleLanguagesMatchRegistry(t *testing.T) {
	var names []string
	for _, lang := range composedLanguages() {
		names = append(names, lang.Name())
	}
	slices.Sort(names)
	want := slices.Clone(generatedLanguages)
	slices.Sort(want)
	if !slices.Equal(names, want) {
		t.Errorf("registered languages = %v, want gazelle_languages %v", names, want)
	}
}

func TestGazelleLanguagesHaveConformanceRows(t *testing.T) {
	for _, name := range generatedLanguages {
		if _, ok := conformanceRows[name]; !ok {
			t.Errorf("%s is in gazelle_languages but has no conformance row", name)
		}
	}
}
