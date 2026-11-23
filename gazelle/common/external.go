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

const ExternalDirective = "dx_external"

type externalParser struct {
	Discover    string
	DefaultRepo string
	Below       func(root, rel string) bool
	Place       func(m externalManifest, rel string) externalManifest
	Parse       func(content []byte, path string) (externalManifest, error)
}

type externalManifest interface {
	Lookup(repo, name string) ([]label.Label, string)
}

var externalParsers = map[string]externalParser{
	"go": {
		Discover: registryEntry("go").Manifest,
		Parse:    parseGoModManifest,
	},
	"maven": {
		Discover:    registryEntry("maven").Lock,
		DefaultRepo: registryEntry("maven").Hub,
		Parse:       parseMavenManifest,
	},
	"python": {
		Discover:    registryEntry("uv").Lock,
		DefaultRepo: registryEntry("uv").Hub,
		Parse:       parseUVLockManifest,
	},
	"pub": {
		Discover:    registryEntry("pub").Lock,
		DefaultRepo: registryEntry("pub").Hub,
		Parse:       parsePubLockManifest,
	},
	"hex": {
		Discover:    registryEntry("hex").Lock,
		DefaultRepo: registryEntry("hex").Hub,
		Parse:       parseHexLockManifest,
	},
	"nuget": {
		Discover:    registryEntry("nuget").Lock,
		DefaultRepo: registryEntry("nuget").Hub,
		Parse:       parseNugetLockManifest,
	},
	"npm": {
		Discover: registryEntry("npm").Manifest,
		Below:    npmBelow,
		Place:    placeNpmManifest,
		Parse:    parseNpmManifest,
	},
}

type externalSource struct {
	Repo     string
	Path     string
	Manifest externalManifest
}

type externalIndex struct {
	rel     string
	sources map[string][]externalSource
	known   *externalKnown
}

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
	externalCacheMu.Lock()
	entry, ok := externalCache[key]
	externalCacheMu.Unlock()
	if ok {
		return entry.manifest, entry.err
	}
	content, err := os.ReadFile(abs)
	if err == nil {
		entry.manifest, entry.err = parser.Parse(StripBOM(content), relPath)
	} else {
		entry.err = err
	}
	externalCacheMu.Lock()
	externalCache[key] = entry
	externalCacheMu.Unlock()
	return entry.manifest, entry.err
}
