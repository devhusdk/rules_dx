package common

import (
	"fmt"
	"slices"
	"strings"
	"sync"

	"github.com/bazelbuild/bazel-gazelle/language"
)

var (
	registryMu sync.Mutex
	registry   []func() language.Language
)

// RegisterLanguage adds a language constructor to the set that dispatch
// composes. Language packages call it from init.
func RegisterLanguage(newLang func() language.Language) {
	registryMu.Lock()
	defer registryMu.Unlock()
	registry = append(registry, newLang)
}

// RegisteredLanguages returns a new instance of every registered language,
// sorted by Name. It panics when two languages share a name.
func RegisteredLanguages() []language.Language {
	registryMu.Lock()
	defer registryMu.Unlock()
	langs := make([]language.Language, 0, len(registry))
	for _, newLang := range registry {
		langs = append(langs, newLang())
	}
	slices.SortFunc(langs, func(a, b language.Language) int { return strings.Compare(a.Name(), b.Name()) })
	for i := 1; i < len(langs); i++ {
		if langs[i-1].Name() == langs[i].Name() {
			panic(fmt.Sprintf("language %q is registered twice", langs[i].Name()))
		}
	}
	return langs
}
