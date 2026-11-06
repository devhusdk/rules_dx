package common

import (
	"fmt"
	"os"
	"path"
	"path/filepath"
	"slices"
	"sort"
	"strings"
	"sync"
	"time"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
)

// ExternalDirective names the directive that points an ecosystem at the
// manifest or lockfile its third-party imports resolve through:
//
//	# gazelle:dx_external <ecosystem> <path|off> [<repo>]
const ExternalDirective = "dx_external"

// externalParser reads one manifest or lockfile of an ecosystem.
type externalParser struct {
	// Discover is the file name that, present in a configured directory,
	// feeds the ecosystem for that directory and everything below it.
	Discover string
	// DefaultRepo is the hub repository a source uses when the directive
	// names none. An empty DefaultRepo means the ecosystem takes no repo.
	DefaultRepo string
	// Below, when set, reports whether a non-root directory may feed the
	// ecosystem. The root needs no such check.
	Below func(root, rel string) bool
	// Place adjusts a discovered manifest for the directory it was found in.
	Place func(m externalManifest, rel string) externalManifest
	Parse    func(content []byte, path string) (externalManifest, error)
	// Sibling names a file next to Discover that ParseWithSibling also reads.
	// It is nil when the file is absent.
	Sibling          string
	ParseWithSibling func(content, sibling []byte, path string) (externalManifest, error)
}

// externalManifest is a parsed source of one ecosystem.
type externalManifest interface {
	// Lookup returns the labels for name under repo, or a hint when the
	// manifest knows why name has no label.
	Lookup(repo, name string) ([]label.Label, string)
}

var externalParsers = map[string]externalParser{
	"go": {
		Discover: "go.mod",
		Parse:    parseGoModManifest,
	},
	"maven": {
		Discover:    "maven_install.json",
		DefaultRepo: "maven",
		Parse:       parseMavenManifest,
	},
	"python": {
		Discover:    "uv.lock",
		DefaultRepo: "pypi",
		Parse:       parseUVLockManifest,
	},
	"pub": {
		Discover:    "pubspec.lock",
		DefaultRepo: "pub_deps",
		Parse:       parsePubLockManifest,
	},
	"dub": {
		Discover:         "dub.json",
		DefaultRepo:      "dub",
		Sibling:          "dub.selections.json",
		ParseWithSibling: parseDubManifest,
	},
	"npm": {
		Discover: "package.json",
		Below:    npmBelow,
		Place:     placeNpmManifest,
		Parse:     parseNpmManifest,
	},
}

// externalSource is one manifest feeding an ecosystem.
type externalSource struct {
	Repo     string
	Path     string
	Manifest externalManifest
}

// externalIndex is the third-party lookup in effect for one directory. It is
// copy-on-write: a child directory never mutates the index it inherited.
type externalIndex struct {
	rel     string
	sources map[string][]externalSource
	known   *externalKnown
}

// externalKnown lists every discovered source, whether in effect or not. The
// root index creates it and every directory shares it.
type externalKnown struct {
	mu    sync.Mutex
	byEco map[string][]externalSource
}

func (k *externalKnown) add(eco string, src externalSource) {
	k.mu.Lock()
	defer k.mu.Unlock()
	for _, have := range k.byEco[eco] {
		if have.Path == src.Path {
			return
		}
	}
	k.byEco[eco] = append(k.byEco[eco], src)
}

func (k *externalKnown) list(eco string) []externalSource {
	k.mu.Lock()
	defer k.mu.Unlock()
	return append([]externalSource(nil), k.byEco[eco]...)
}

func inheritedExternal(c *config.Config) *externalIndex {
	if c == nil {
		return nil
	}
	idx, _ := c.Exts[ExternalDirective].(*externalIndex)
	return idx
}

// ConfigureExternal records the third-party sources for rel: discovered
// well-known files in the directory, then dx_external directives. It is
// idempotent per rel, so every language may call it and errors are reported
// once.
func ConfigureExternal(c *config.Config, rel string, f *rule.File) []string {
	parent := inheritedExternal(c)
	if parent != nil && parent.rel == rel {
		return nil
	}
	var sources map[string][]externalSource
	known := &externalKnown{byEco: map[string][]externalSource{}}
	if parent != nil {
		sources = parent.sources
		known = parent.known
	}
	cloned := false
	set := func(eco string, list []externalSource) {
		if !cloned {
			next := make(map[string][]externalSource, len(sources)+1)
			for k, v := range sources {
				next[k] = v
			}
			sources = next
			cloned = true
		}
		if len(list) == 0 {
			delete(sources, eco)
			return
		}
		sources[eco] = list
	}
	ecos := make([]string, 0, len(externalParsers))
	for eco := range externalParsers {
		ecos = append(ecos, eco)
	}
	sort.Strings(ecos)
	for _, eco := range ecos {
		parser := externalParsers[eco]
		if rel != "" && parser.Below != nil && !parser.Below(c.RepoRoot, rel) {
			continue
		}
		relPath := path.Join(rel, parser.Discover)
		abs := filepath.Join(c.RepoRoot, filepath.FromSlash(relPath))
		if info, err := os.Stat(abs); err != nil || info.IsDir() {
			continue
		}
		manifest, err := loadExternal(parser, abs, relPath)
		if err != nil {
			// A discovered file that does not parse is left alone; only a
			// file a directive names is an error.
			continue
		}
		if parser.Place != nil {
			manifest = parser.Place(manifest, rel)
		}
		src := externalSource{Repo: parser.DefaultRepo, Path: relPath, Manifest: manifest}
		known.add(eco, src)
		set(eco, []externalSource{src})
	}
	var errs []string
	if f != nil {
		declared := map[string]bool{}
		for _, d := range f.Directives {
			if d.Key != ExternalDirective {
				continue
			}
			fields := strings.Fields(d.Value)
			if len(fields) < 2 || len(fields) > 3 {
				errs = append(errs, fmt.Sprintf("dx_external: //%s: malformed # gazelle:dx_external %s; want <ecosystem> <path|off> [<repo>]", rel, d.Value))
				continue
			}
			eco, file := fields[0], fields[1]
			parser, ok := externalParsers[eco]
			if !ok {
				errs = append(errs, fmt.Sprintf("dx_external: //%s: unsupported ecosystem %q; supported: %s", rel, eco, strings.Join(ecos, ", ")))
				continue
			}
			repo := parser.DefaultRepo
			if len(fields) == 3 {
				if parser.DefaultRepo == "" {
					errs = append(errs, fmt.Sprintf("dx_external: //%s: ecosystem %s takes no repo; its repos are named from module paths", rel, eco))
					continue
				}
				repo = fields[2]
			}
			var list []externalSource
			if declared[eco] {
				list = sources[eco]
			}
			declared[eco] = true
			if file == "off" {
				if len(fields) == 3 {
					errs = append(errs, fmt.Sprintf("dx_external: //%s: malformed # gazelle:dx_external %s; off takes no repo", rel, d.Value))
				}
				set(eco, list)
				continue
			}
			relPath := path.Clean(strings.TrimPrefix(file, "//"))
			abs := filepath.Join(c.RepoRoot, filepath.FromSlash(relPath))
			manifest, err := loadExternal(parser, abs, relPath)
			if err != nil {
				errs = append(errs, fmt.Sprintf("dx_external: //%s: %s %s: %v", rel, eco, relPath, err))
				set(eco, list)
				continue
			}
			set(eco, append(append([]externalSource(nil), list...), externalSource{Repo: repo, Path: relPath, Manifest: manifest}))
		}
	}
	c.Exts[ExternalDirective] = &externalIndex{rel: rel, sources: sources, known: known}
	return errs
}

func hasFile(root, rel string) bool {
	info, err := os.Stat(filepath.Join(root, filepath.FromSlash(rel)))
	return err == nil && !info.IsDir()
}

// ExternalLabels returns the labels the ecosystem's sources give name. More
// than one label is ambiguous. With no label, hint says why when a source
// knows.
func ExternalLabels(c *config.Config, ecosystem, name string) ([]label.Label, string) {
	idx := inheritedExternal(c)
	if idx == nil {
		return nil, ""
	}
	var out []label.Label
	seen := map[label.Label]bool{}
	hint := ""
	for _, src := range idx.sources[ecosystem] {
		labels, h := src.Manifest.Lookup(src.Repo, name)
		for _, l := range labels {
			if !seen[l] {
				seen[l] = true
				out = append(out, l)
			}
		}
		if hint == "" && h != "" {
			hint = h + " (" + src.Path + ")"
		}
	}
	if len(out) > 0 {
		return out, ""
	}
	return nil, hint
}

// ExternalElsewhere names the dx_external directive that would resolve name:
// a discovered source of the ecosystem that provides it but is not in effect
// for the configured directory. It is empty when there is none.
func ExternalElsewhere(c *config.Config, ecosystem, name string) string {
	idx := inheritedExternal(c)
	if idx == nil {
		return ""
	}
	for _, src := range idx.known.list(ecosystem) {
		if slices.ContainsFunc(idx.sources[ecosystem], func(have externalSource) bool { return have.Path == src.Path }) {
			continue
		}
		if labels, _ := src.Manifest.Lookup(src.Repo, name); len(labels) == 0 {
			continue
		}
		directive := ExternalDirective + " " + ecosystem + " " + src.Path
		if src.Repo != "" {
			directive += " " + src.Repo
		}
		return fmt.Sprintf("%s provides it; add \"# gazelle:%s\" to BUILD.bazel", src.Path, directive)
	}
	return ""
}

// ExternalSources returns the repo-relative paths feeding ecosystem for the
// configured directory.
func ExternalSources(c *config.Config, ecosystem string) []string {
	idx := inheritedExternal(c)
	if idx == nil {
		return nil
	}
	var out []string
	for _, src := range idx.sources[ecosystem] {
		out = append(out, src.Path)
	}
	return out
}

type externalCacheKey struct {
	eco  string
	path string
	size int64
	mod  time.Time
	sib  string
}

type externalCacheEntry struct {
	manifest externalManifest
	err      error
}

var (
	externalCacheMu sync.Mutex
	externalCache   = map[externalCacheKey]externalCacheEntry{}
)

func loadExternal(parser externalParser, abs, relPath string) (externalManifest, error) {
	info, err := os.Stat(abs)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, fmt.Errorf("no such file")
		}
		return nil, err
	}
	if info.IsDir() {
		return nil, fmt.Errorf("is a directory")
	}
	key := externalCacheKey{eco: parser.Discover, path: abs, size: info.Size(), mod: info.ModTime()}
	var sibling []byte
	if parser.Sibling != "" {
		if sibling, err = os.ReadFile(filepath.Join(filepath.Dir(abs), parser.Sibling)); err == nil {
			key.sib = string(sibling)
		}
	}
	externalCacheMu.Lock()
	entry, ok := externalCache[key]
	externalCacheMu.Unlock()
	if ok {
		return entry.manifest, entry.err
	}
	content, err := os.ReadFile(abs)
	if err == nil {
		if parser.ParseWithSibling != nil {
			entry.manifest, entry.err = parser.ParseWithSibling(StripBOM(content), sibling, relPath)
		} else {
			entry.manifest, entry.err = parser.Parse(StripBOM(content), relPath)
		}
	} else {
		entry.err = err
	}
	externalCacheMu.Lock()
	externalCache[key] = entry
	externalCacheMu.Unlock()
	return entry.manifest, entry.err
}
