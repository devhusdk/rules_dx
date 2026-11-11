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

// cpanCandidates maps Text::CSV::Foo to Text-CSV-Foo, Text-CSV, Text.
func cpanCandidates(name string) []string {
	parts := strings.Split(name, "::")
	var out []string
	for n := len(parts); n > 0; n-- {
		out = append(out, strings.Join(parts[:n], "-"))
	}
	return out
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

func parseCpanLockManifest(content []byte, path string) (externalManifest, error) {
	var lock map[string]json.RawMessage
	if err := json.Unmarshal(content, &lock); err != nil {
		return nil, err
	}
	m := &namedLockManifest{entry: registryEntry("cpan"), packages: map[string]bool{}, candidates: cpanCandidates}
	for name := range lock {
		m.packages[name] = true
	}
	return m, nil
}

var hexLockEntry = regexp.MustCompile(`(?m)^\s*"([^"]+)":\s*\{:hex,`)

func parseHexLockManifest(content []byte, path string) (externalManifest, error) {
	m := &namedLockManifest{entry: registryEntry("hex"), packages: map[string]bool{}, candidates: hexCandidates}
	for _, match := range hexLockEntry.FindAllSubmatch(content, -1) {
		m.packages[string(match[1])] = true
	}
	if len(m.packages) == 0 && !strings.Contains(string(content), "%{") {
		return nil, fmt.Errorf("%s is not a %s", path, m.entry.Lock)
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

var paketLockEntry = regexp.MustCompile(`(?m)^ {4}(\S+) \(`)

func parsePaketLockManifest(content []byte, path string) (externalManifest, error) {
	m := &namedLockManifest{entry: registryEntry("nuget"), packages: map[string]bool{}, candidates: nugetCandidates, perPackage: true}
	for _, match := range paketLockEntry.FindAllSubmatch(content, -1) {
		m.packages[strings.ToLower(string(match[1]))] = true
	}
	if len(m.packages) == 0 && !strings.Contains(string(content), "NUGET") {
		return nil, fmt.Errorf("%s is not a %s", path, m.entry.Lock)
	}
	return m, nil
}
