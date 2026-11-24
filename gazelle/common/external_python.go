package common

import (
	"regexp"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
)

var pyNameRun = regexp.MustCompile(`[-_.]+`)

func pythonNormalize(name string) string {
	return strings.ToLower(pyNameRun.ReplaceAllString(name, "-"))
}

func pythonHubName(name string) string {
	return strings.ToLower(pyNameRun.ReplaceAllString(name, "_"))
}

type pythonManifest struct {
	names map[string]bool
}

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

func (m *pythonManifest) Lookup(repo, name string) ([]label.Label, string) {
	top, _, _ := strings.Cut(name, ".")
	dist := pythonNormalize(top)
	if !m.names[dist] {
		return nil, ""
	}
	hub := pythonHubName(top)
	return []label.Label{{Repo: repo, Pkg: hub, Name: hub}}, ""
}
