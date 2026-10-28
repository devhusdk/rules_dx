package julia

import (
	"fmt"
	"regexp"
	"sort"
	"unicode"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/pelletier/go-toml/v2"
)

var uuidPattern = regexp.MustCompile(`^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$`)

// Project holds the facts dx generate reads from a Project.toml.
type Project struct {
	Name string
	UUID string
	// Deps and Extras map a package name to its UUID.
	Deps   map[string]string
	Extras map[string]string
	// Test lists the [targets].test names.
	Test []string
}

type projectTOML struct {
	Name    string              `toml:"name"`
	UUID    string              `toml:"uuid"`
	Deps    map[string]any      `toml:"deps"`
	Extras  map[string]any      `toml:"extras"`
	Targets map[string][]string `toml:"targets"`
}

// IsIdentifier reports whether name is a valid Julia package name.
func IsIdentifier(name string) bool {
	if name == "" {
		return false
	}
	for i, r := range name {
		switch {
		case r == '_' || unicode.IsLetter(r):
		case i > 0 && (unicode.IsDigit(r) || r == '!'):
		default:
			return false
		}
	}
	return true
}

// ParseProject reads path's content. A Project.toml without a name is an
// environment and parses to an empty Name. Errors name path.
func ParseProject(path string, content []byte) (Project, error) {
	var raw projectTOML
	if err := toml.Unmarshal(common.StripBOM(content), &raw); err != nil {
		return Project{}, fmt.Errorf("%s: invalid TOML: %v", path, err)
	}
	if raw.Name != "" && !IsIdentifier(raw.Name) {
		return Project{}, fmt.Errorf("%s: name %q is not a Julia identifier", path, raw.Name)
	}
	p := Project{Name: raw.Name, UUID: raw.UUID}
	var err error
	if p.Deps, err = uuidTable(path, "deps", raw.Deps); err != nil {
		return Project{}, err
	}
	if p.Extras, err = uuidTable(path, "extras", raw.Extras); err != nil {
		return Project{}, err
	}
	p.Test = raw.Targets["test"]
	for _, name := range p.Test {
		if _, ok := p.Extras[name]; !ok {
			return Project{}, fmt.Errorf("%s: [targets].test names %q, which is not in [extras]", path, name)
		}
	}
	return p, nil
}

func uuidTable(path, table string, raw map[string]any) (map[string]string, error) {
	out := make(map[string]string, len(raw))
	for name, value := range raw {
		id, ok := value.(string)
		if !ok || !uuidPattern.MatchString(id) {
			return nil, fmt.Errorf("%s: [%s] %s must be a UUID string", path, table, name)
		}
		out[name] = id
	}
	return out, nil
}

// TestDeps returns the sorted package names the tests need, excluding the
// package itself: test/Project.toml [deps] when it exists (override), else
// [extras] filtered by [targets].test.
func (p Project) TestDeps(override *Project) map[string]string {
	out := make(map[string]string)
	if override != nil {
		for name, id := range override.Deps {
			out[name] = id
		}
	} else {
		for _, name := range p.Test {
			out[name] = p.Extras[name]
		}
	}
	delete(out, p.Name)
	return out
}

func sortedKeys(m map[string]string) []string {
	out := make([]string, 0, len(m))
	for k := range m {
		out = append(out, k)
	}
	sort.Strings(out)
	return out
}
