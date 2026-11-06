package common

import (
	"encoding/json"
	"path"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
)

// npmManifest holds the packages a package.json declares.
type npmManifest struct {
	packages map[string]bool
	// dir is the package holding the pnpm lock the packages link from.
	dir string
}

// npmBelow lets a directory feed npm when it holds a pnpm lock or is a
// workspace importer of one.
func npmBelow(root, rel string) bool {
	return hasFile(root, path.Join(rel, "pnpm-lock.yaml")) || IsPnpmWorkspaceMember(root, rel)
}

func placeNpmManifest(m externalManifest, rel string) externalManifest {
	placed := *m.(*npmManifest)
	placed.dir = rel
	return &placed
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

// NpmPackageName cuts a module specifier down to its package name, keeping
// the @scope/name pair.
func NpmPackageName(spec string) string {
	parts := strings.Split(spec, "/")
	if strings.HasPrefix(spec, "@") && len(parts) >= 2 {
		return parts[0] + "/" + parts[1]
	}
	return parts[0]
}

// Lookup resolves a declared package to its //<lockdir>:node_modules/<pkg> target.
func (m *npmManifest) Lookup(_, name string) ([]label.Label, string) {
	pkg := NpmPackageName(name)
	if !m.packages[pkg] {
		return nil, ""
	}
	return []label.Label{{Pkg: m.dir, Name: "node_modules/" + pkg}}, ""
}
