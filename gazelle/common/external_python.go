package common

import (
	"regexp"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
)

var pyNameRun = regexp.MustCompile(`[-_.]+`)

// pythonNormalize is the PEP 503 normal form of a distribution name.
func pythonNormalize(name string) string {
	return strings.ToLower(pyNameRun.ReplaceAllString(name, "-"))
}

// pythonManifest holds the registry distributions a uv.lock pins.
type pythonManifest struct {
	names map[string]bool
}

// parseUVLockManifest reads the [[package]] tables of a uv.lock. Packages
// whose source is the workspace itself (virtual or editable) are skipped.
func parseUVLockManifest(content []byte, _ string) (externalManifest, error) {
	m := &pythonManifest{names: map[string]bool{}}
	var name string
	local := false
	inPackage := false
	flush := func() {
		if inPackage && name != "" && !local {
			m.names[pythonNormalize(name)] = true
		}
		name, local, inPackage = "", false, false
	}
	for _, line := range strings.Split(string(content), "\n") {
		line = strings.TrimSpace(line)
		switch {
		case line == "[[package]]":
			flush()
			inPackage = true
		case strings.HasPrefix(line, "["):
			flush()
		case inPackage && strings.HasPrefix(line, "name = "):
			name = strings.Trim(strings.TrimPrefix(line, "name = "), `"`)
		case inPackage && strings.HasPrefix(line, "source = ") &&
			(strings.Contains(line, "virtual =") || strings.Contains(line, "editable =") || strings.Contains(line, "directory =")):
			local = true
		}
	}
	flush()
	return m, nil
}

// Lookup resolves the top-level module of name to the distribution of the
// same normalized name. Distributions whose import name differs from their
// name need a resolve mapping.
func (m *pythonManifest) Lookup(repo, name string) ([]label.Label, string) {
	top, _, _ := strings.Cut(name, ".")
	dist := pythonNormalize(top)
	if !m.names[dist] {
		return nil, ""
	}
	return []label.Label{{Repo: repo, Pkg: dist, Name: dist}}, ""
}
