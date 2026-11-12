package common

import (
	_ "embed"
	"encoding/json"
	"fmt"
	"slices"
	"sort"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
)

//go:embed lock_registry.json
var lockRegistryJSON []byte

// lockRegistryEntry is one dx lock ecosystem of the generated registry.
type lockRegistryEntry struct {
	Ecosystem string `json:"ecosystem"`
	Manifest  string `json:"manifest"`
	Lock      string `json:"lock"`
	Hub       string `json:"hub"`
	Imports   string `json:"imports"`
}

var registryEntries = func() map[string]lockRegistryEntry {
	var list []lockRegistryEntry
	if err := json.Unmarshal(lockRegistryJSON, &list); err != nil {
		panic("lock_registry.json: " + err.Error())
	}
	entries := map[string]lockRegistryEntry{}
	for _, e := range list {
		entries[e.Ecosystem] = e
	}
	return entries
}()

// registryEntry returns the registry entry of an ecosystem.
func registryEntry(ecosystem string) lockRegistryEntry {
	e, ok := registryEntries[ecosystem]
	if !ok {
		panic("lock_registry.json: no entry for " + ecosystem)
	}
	return e
}

// ManifestName returns the manifest file name the registry gives an ecosystem.
func ManifestName(ecosystem string) string {
	return registryEntry(ecosystem).Manifest
}

// LockName returns the lock file name the registry gives an ecosystem.
func LockName(ecosystem string) string {
	return registryEntry(ecosystem).Lock
}

// lockRegistry lists every ecosystem whose imports resolve through the
// provides of its lock.
var lockRegistry = func() []lockRegistryEntry {
	var entries []lockRegistryEntry
	for _, e := range registryEntries {
		if _, ok := externalParsers[e.Ecosystem]; ok {
			continue
		}
		if e.Imports == "provides" {
			entries = append(entries, e)
		}
	}
	sort.Slice(entries, func(i, j int) bool { return entries[i].Ecosystem < entries[j].Ecosystem })
	return entries
}()

func init() {
	for _, e := range lockRegistry {
		externalParsers[e.Ecosystem] = externalParser{
			Discover:    e.Lock,
			DefaultRepo: e.Hub,
			Place:       placeLockManifest,
			Parse:       parseLockManifest(e),
		}
	}
}

// lockManifest holds the packages a dx lock names and what each provides.
type lockManifest struct {
	entry    lockRegistryEntry
	provides map[string][]string
	// repo is the hub repository of a set that is not the root.
	repo string
}

func parseLockManifest(entry lockRegistryEntry) func([]byte, string) (externalManifest, error) {
	return func(content []byte, path string) (externalManifest, error) {
		var lock struct {
			Ecosystem string `json:"ecosystem"`
			Packages  []struct {
				Name     string   `json:"name"`
				Provides []string `json:"provides"`
			} `json:"packages"`
		}
		if err := json.Unmarshal(content, &lock); err != nil {
			return nil, err
		}
		if lock.Ecosystem != entry.Ecosystem {
			return nil, fmt.Errorf("%s is a lock of ecosystem %q, not %q", path, lock.Ecosystem, entry.Ecosystem)
		}
		m := &lockManifest{entry: entry, provides: map[string][]string{}}
		for _, p := range lock.Packages {
			for _, imp := range p.Provides {
				if !slices.Contains(m.provides[imp], p.Name) {
					m.provides[imp] = append(m.provides[imp], p.Name)
				}
			}
		}
		return m, nil
	}
}

// placeLockManifest gives a set below the root the hub <hub>_<dir>.
func placeLockManifest(m externalManifest, rel string) externalManifest {
	lm, ok := m.(*lockManifest)
	if !ok || rel == "" {
		return m
	}
	placed := *lm
	placed.repo = lm.entry.Hub + "_" + strings.NewReplacer("/", "_", "-", "_", ".", "_").Replace(rel)
	return &placed
}

// Lookup resolves name by the longest provides entry that is name or a path
// or namespace prefix of it. Every package providing that entry is returned.
func (m *lockManifest) Lookup(repo, name string) ([]label.Label, string) {
	if m.repo != "" && repo == m.entry.Hub {
		repo = m.repo
	}
	candidate := name
	for {
		if pkgs, ok := m.provides[candidate]; ok {
			labels := make([]label.Label, 0, len(pkgs))
			for _, pkg := range pkgs {
				labels = append(labels, label.Label{Repo: repo, Pkg: pkg, Name: pkg})
			}
			return labels, ""
		}
		i := strings.LastIndexAny(candidate, `/.\:`)
		if i < 0 {
			break
		}
		candidate = candidate[:i]
	}
	return nil, fmt.Sprintf("no package in %s provides %s; add it to %s", m.entry.Lock, name, m.entry.Manifest)
}

func lockEcosystems() []string {
	var out []string
	for _, e := range lockRegistry {
		out = append(out, e.Ecosystem)
	}
	sort.Strings(out)
	return out
}
