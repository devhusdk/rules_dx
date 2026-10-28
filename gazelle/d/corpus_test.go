package d

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// TestCorpus parses every D source of the pinned tool repositories. Each file
// must parse without error and declare the module its path spells, so a
// grammar regression in a dependency bump fails here instead of in generation.
func TestCorpus(t *testing.T) {
	list := os.Getenv("D_CORPUS_FILES")
	if list == "" {
		t.Skip("D_CORPUS_FILES is not set; run through Bazel")
	}
	files := strings.Fields(list)
	if len(files) < 100 {
		t.Fatalf("corpus holds %d files, want at least 100", len(files))
	}
	for _, file := range files {
		content, err := os.ReadFile(filepath.Join(os.Getenv("TEST_SRCDIR"), filepath.FromSlash(file)))
		if err != nil {
			t.Fatal(err)
		}
		info, err := Analyze(content)
		if err != nil {
			t.Errorf("%s: %v", file, err)
			continue
		}
		if info.Module == "" {
			continue
		}
		want := strings.ReplaceAll(info.Module, ".", "/")
		got := strings.TrimSuffix(filepath.ToSlash(file), ".d")
		got = strings.TrimSuffix(got, "/package")
		if got != want && !strings.HasSuffix(got, "/"+want) {
			t.Errorf("%s declares module %s, which its path does not spell", file, info.Module)
		}
	}
}
