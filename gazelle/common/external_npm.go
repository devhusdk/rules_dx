package common

import (
	"encoding/json"
	"path"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
)

var npmLockFile = LockName("npm")

type npmManifest struct {
	packages map[string]bool
	dir      string
}

func npmBelow(root, rel string) bool {
	return hasFile(root, path.Join(rel, npmLockFile)) || IsPnpmWorkspaceMember(root, rel)
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

func NpmPackageName(spec string) string {
	parts := strings.Split(spec, "/")
	if strings.HasPrefix(spec, "@") && len(parts) >= 2 {
		return parts[0] + "/" + parts[1]
	}
	return parts[0]
}

func (m *npmManifest) Lookup(_, name string) ([]label.Label, string) {
	pkg := NpmPackageName(name)
	if !m.packages[pkg] {
		return nil, ""
	}
	return []label.Label{{Pkg: m.dir, Name: "node_modules/" + pkg}}, ""
}
