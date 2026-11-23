package common

import (
	"fmt"
	"sort"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
)

type pubPackage struct {
	dependency string
	source     string
}

type pubLockManifest struct {
	packages map[string]pubPackage
}

func parsePubLockManifest(content []byte, _ string) (externalManifest, error) {
	m := &pubLockManifest{packages: map[string]pubPackage{}}
	inPackages, seenPackages := false, false
	current := ""
	for i, raw := range strings.Split(string(content), "\n") {
		line := strings.TrimRight(raw, " \t\r")
		trimmed := strings.TrimSpace(line)
		if trimmed == "" || strings.HasPrefix(trimmed, "#") {
			continue
		}
		indent := len(line) - len(strings.TrimLeft(line, " "))
		if strings.Contains(line[:indent], "\t") {
			return nil, fmt.Errorf("line %d: tab indentation", i+1)
		}
		key, value, ok := strings.Cut(trimmed, ":")
		if !ok {
			return nil, fmt.Errorf("line %d: expected key: value", i+1)
		}
		value = strings.Trim(strings.TrimSpace(value), `"'`)
		switch {
		case indent == 0:
			inPackages = key == "packages"
			seenPackages = seenPackages || inPackages
			current = ""
		case !inPackages:
		case indent == 2:
			if value != "" {
				return nil, fmt.Errorf("line %d: package %s must be a mapping", i+1, key)
			}
			current = strings.Trim(key, `"'`)
			if _, dup := m.packages[current]; dup {
				return nil, fmt.Errorf("line %d: duplicate package %s", i+1, current)
			}
			m.packages[current] = pubPackage{}
		case indent == 4:
			if current == "" {
				return nil, fmt.Errorf("line %d: field outside a package", i+1)
			}
			pkg := m.packages[current]
			switch key {
			case "dependency":
				pkg.dependency = value
			case "source":
				pkg.source = value
			}
			m.packages[current] = pkg
		}
	}
	if !seenPackages {
		return nil, fmt.Errorf("no packages: section")
	}
	var bad []string
	for name, pkg := range m.packages {
		if pkg.dependency == "" || pkg.source == "" {
			bad = append(bad, name)
		}
	}
	if len(bad) > 0 {
		sort.Strings(bad)
		return nil, fmt.Errorf("packages lack dependency or source: %s", strings.Join(bad, ", "))
	}
	return m, nil
}

func (m *pubLockManifest) Lookup(repo, name string) ([]label.Label, string) {
	pkg, ok := m.packages[name]
	switch {
	case !ok:
		return nil, "package " + name + " is not in the lock; declare it in the pub manifest and run pub get"
	case pkg.source == "path" || pkg.source == "sdk":
		return nil, "package " + name + " comes from a " + pkg.source + " source, not the pub hub"
	case pkg.dependency == "transitive":
		return nil, "package " + name + " is only a transitive dependency; declare it in the pub manifest"
	}
	return []label.Label{{Repo: repo, Name: name}}, ""
}
