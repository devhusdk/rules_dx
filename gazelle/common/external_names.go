package common

import (
	"encoding/json"
	"fmt"
	"regexp"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
)

// namedLockManifest holds the package names of a lock whose ecosystem maps an
// import to a package name by a fixed rule.
type namedLockManifest struct {
	entry    lockRegistryEntry
	packages map[string]bool
	// candidates lists the package names an import can mean, best first.
	candidates func(name string) []string
	// perPackage places each package in its own Bazel package.
	perPackage bool
}

// Lookup resolves name to the first candidate package the lock holds.
func (m *namedLockManifest) Lookup(repo, name string) ([]label.Label, string) {
	for _, pkg := range m.candidates(name) {
		if m.packages[pkg] {
			if m.perPackage {
				return []label.Label{{Repo: repo, Pkg: pkg, Name: pkg}}, ""
			}
			return []label.Label{{Repo: repo, Name: pkg}}, ""
		}
	}
	return nil, fmt.Sprintf("no package in %s provides %s; add it to %s", m.entry.Lock, name, m.entry.Manifest)
}

var (
	upperRun   = regexp.MustCompile(`([A-Z]+)([A-Z][a-z])`)
	lowerUpper = regexp.MustCompile(`([a-z\d])([A-Z])`)
)

// hexCandidates maps Phoenix.LiveView.Foo to phoenix_live_view_foo,
// phoenix_live_view, phoenix.
func hexCandidates(name string) []string {
	parts := strings.Split(name, ".")
	var out []string
	for n := len(parts); n > 0; n-- {
		var words []string
		for _, p := range parts[:n] {
			p = upperRun.ReplaceAllString(p, "${1}_${2}")
			p = lowerUpper.ReplaceAllString(p, "${1}_${2}")
			words = append(words, strings.ToLower(p))
		}
		out = append(out, strings.Join(words, "_"))
	}
	return out
}

func parseHexLockManifest(content []byte, path string) (externalManifest, error) {
	entry := registryEntry("hex")
	var lock struct {
		Ecosystem string `json:"ecosystem"`
		Packages  []struct {
			Name string `json:"name"`
		} `json:"packages"`
	}
	if err := json.Unmarshal(content, &lock); err != nil {
		return nil, err
	}
	if lock.Ecosystem != entry.Ecosystem {
		return nil, fmt.Errorf("%s is a lock of ecosystem %q, not %q", path, lock.Ecosystem, entry.Ecosystem)
	}
	m := &namedLockManifest{entry: entry, packages: map[string]bool{}, candidates: hexCandidates, perPackage: true}
	for _, p := range lock.Packages {
		m.packages[p.Name] = true
	}
	return m, nil
}

// nugetCandidates maps Newtonsoft.Json.Linq to newtonsoft.json.linq,
// newtonsoft.json, newtonsoft.
func nugetCandidates(name string) []string {
	parts := strings.Split(strings.ToLower(name), ".")
	var out []string
	for n := len(parts); n > 0; n-- {
		out = append(out, strings.Join(parts[:n], "."))
	}
	return out
}

var paketNugetSection = regexp.MustCompile(`(?m)^NUGET\n((?:[ \t]+.*\n?|\n)*)`)

var paketLockEntry = regexp.MustCompile(`(?m)^ {4}(\S+) \(`)

func parsePaketLockManifest(content []byte, path string) (externalManifest, error) {
	m := &namedLockManifest{entry: registryEntry("nuget"), packages: map[string]bool{}, candidates: nugetCandidates, perPackage: true}
	for _, section := range paketNugetSection.FindAllSubmatch(content, -1) {
		for _, match := range paketLockEntry.FindAllSubmatch(section[1], -1) {
			m.packages[strings.ToLower(string(match[1]))] = true
		}
	}
	if len(m.packages) == 0 && !strings.Contains(string(content), "NUGET") {
		return nil, fmt.Errorf("%s is not a %s", path, m.entry.Lock)
	}
	return m, nil
}
