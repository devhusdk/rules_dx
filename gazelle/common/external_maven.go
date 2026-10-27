package common

import (
	"encoding/json"
	"fmt"
	"slices"
	"sort"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
)

var mavenPackagingTypes = []string{
	"aar", "bundle", "dll", "dylib", "eclipse-plugin", "exe", "hk2-jar", "jar", "json",
	"maven-plugin", "orbit", "pom", "scala-jar", "so", "test-jar",
}

// mavenManifest maps each JVM package to the artifact targets that ship it.
type mavenManifest struct {
	packages map[string][]string
}

func parseMavenManifest(content []byte, _ string) (externalManifest, error) {
	var lock struct {
		Packages map[string][]string `json:"packages"`
	}
	if err := json.Unmarshal(content, &lock); err != nil {
		return nil, err
	}
	if lock.Packages == nil {
		return nil, fmt.Errorf("no packages map; pin it with rules_jvm_external lock format v2 or later")
	}
	m := &mavenManifest{packages: map[string][]string{}}
	for coord, pkgs := range lock.Packages {
		target := MavenTargetName(coord)
		for _, pkg := range pkgs {
			if !slices.Contains(m.packages[pkg], target) {
				m.packages[pkg] = append(m.packages[pkg], target)
			}
		}
	}
	for _, targets := range m.packages {
		sort.Strings(targets)
	}
	return m, nil
}

// MavenTargetName is the versionless target rules_jvm_external gives a
// group:artifact[:packaging[:classifier]] coordinate.
func MavenTargetName(coord string) string {
	parts := strings.Split(coord, ":")
	if len(parts) < 2 {
		return mavenEscape(coord)
	}
	out := parts[0] + ":" + parts[1]
	if len(parts) >= 4 {
		if packaging := parts[2]; packaging != "" && !slices.Contains(mavenPackagingTypes, packaging) {
			out += ":" + packaging
		}
		if classifier := parts[3]; classifier != "" && classifier != "sources" && classifier != "natives" {
			out += ":" + classifier
		}
	}
	return mavenEscape(out)
}

func mavenEscape(s string) string {
	s = strings.NewReplacer(".", "_", "-", "_", ":", "_", "/", "_", "+", "_", "$", "_", "[", "", "]", "").Replace(s)
	s, _, _ = strings.Cut(s, ",")
	return s
}

// Lookup tries the import itself, then its enclosing classes and own package.
func (m *mavenManifest) Lookup(repo, name string) ([]label.Label, string) {
	for _, key := range append([]string{name}, JVMCandidates(name)...) {
		targets := m.packages[key]
		if len(targets) == 0 {
			continue
		}
		labels := make([]label.Label, 0, len(targets))
		for _, t := range targets {
			labels = append(labels, label.New(repo, "", t))
		}
		return labels, ""
	}
	return nil, ""
}
