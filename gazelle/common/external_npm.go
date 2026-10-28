package common

import (
	"encoding/json"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
)

// npmManifest holds the packages a package.json declares.
type npmManifest struct {
	packages map[string]bool
}

func parseNpmManifest(content []byte, _ string) (externalManifest, error) {
	var pkg struct {
		Dependencies         map[string]json.RawMessage `json:"dependencies"`
		DevDependencies      map[string]json.RawMessage `json:"devDependencies"`
		OptionalDependencies map[string]json.RawMessage `json:"optionalDependencies"`
		PeerDependencies     map[string]json.RawMessage `json:"peerDependencies"`
	}
	if err := json.Unmarshal(content, &pkg); err != nil {
		return nil, err
	}
	m := &npmManifest{packages: map[string]bool{}}
	for _, deps := range []map[string]json.RawMessage{pkg.Dependencies, pkg.DevDependencies, pkg.OptionalDependencies, pkg.PeerDependencies} {
		for name := range deps {
			m.packages[name] = true
		}
	}
	return m, nil
}

// npmPackageName cuts a module specifier down to its package name.
func npmPackageName(spec string) string {
	parts := strings.Split(spec, "/")
	if strings.HasPrefix(spec, "@") && len(parts) >= 2 {
		return parts[0] + "/" + parts[1]
	}
	return parts[0]
}

// Lookup resolves a declared package to its //:node_modules/<pkg> target.
func (m *npmManifest) Lookup(_, name string) ([]label.Label, string) {
	pkg := npmPackageName(name)
	if !m.packages[pkg] {
		return nil, ""
	}
	return []label.Label{{Name: "node_modules/" + pkg}}, ""
}
