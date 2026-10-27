package rust

import (
	"errors"
	"fmt"
	"regexp"
	"sort"
	"strconv"
	"strings"

	"github.com/pelletier/go-toml/v2"
)

type cargoTOML struct {
	Package           *cargoTOMLPackage            `toml:"package"`
	Lib               *cargoTOMLTarget             `toml:"lib"`
	Bin               []cargoTOMLTarget            `toml:"bin"`
	Test              []cargoTOMLTarget            `toml:"test"`
	Example           []cargoTOMLTarget            `toml:"example"`
	Bench             []cargoTOMLTarget            `toml:"bench"`
	Workspace         *cargoTOMLWorkspace          `toml:"workspace"`
	Dependencies      map[string]any               `toml:"dependencies"`
	DevDependencies   map[string]any               `toml:"dev-dependencies"`
	BuildDependencies map[string]any               `toml:"build-dependencies"`
	Target            map[string]cargoTOMLPlatform `toml:"target"`
}

type cargoTOMLPlatform struct {
	Dependencies      map[string]any `toml:"dependencies"`
	DevDependencies   map[string]any `toml:"dev-dependencies"`
	BuildDependencies map[string]any `toml:"build-dependencies"`
}

type cargoTOMLWorkspace struct {
	Package struct {
		Version string `toml:"version"`
		Edition string `toml:"edition"`
	} `toml:"package"`
	Dependencies map[string]any `toml:"dependencies"`
}

type cargoTOMLPackage struct {
	Name      string `toml:"name"`
	Workspace string `toml:"workspace"`
	// Version, Edition and Build are a string, a bool, or a table.
	Version any `toml:"version"`
	Edition any `toml:"edition"`
	Build   any `toml:"build"`
}

// cargoTOMLTarget uses pointers so a present field differs from a missing one.
type cargoTOMLTarget struct {
	Name             string    `toml:"name"`
	Path             *string   `toml:"path"`
	Harness          *bool     `toml:"harness"`
	Test             *bool     `toml:"test"`
	Bench            *bool     `toml:"bench"`
	ProcMacro        *bool     `toml:"proc-macro"`
	CrateType        *[]string `toml:"crate-type"`
	RequiredFeatures *[]string `toml:"required-features"`
}

type cargoDepEntry struct {
	key       string
	cfg       string
	keyPath   string
	version   string
	depPath   *string
	pkg       *string
	workspace *bool
}

var cargoDepTables = []string{"dependencies", "dev-dependencies", "build-dependencies", "workspace"}

func decodeCargoTOML(manifestPath string, content []byte) (*cargoTOML, error) {
	var doc cargoTOML
	if err := toml.Unmarshal(content, &doc); err != nil {
		return nil, cargoTOMLError(manifestPath, err)
	}
	return &doc, nil
}

var tomlMismatch = regexp.MustCompile(`^cannot decode TOML (.+?) into `)

// cargoTOMLError turns a go-toml error into rust: <path>:<row>: <key>: <msg>
// without leaking Go type names.
func cargoTOMLError(manifestPath string, err error) error {
	var de *toml.DecodeError
	if !errors.As(err, &de) {
		return fmt.Errorf("rust: %s: %s", manifestPath, strings.TrimPrefix(err.Error(), "toml: "))
	}
	row, _ := de.Position()
	key := []string(de.Key())
	msg := strings.TrimPrefix(de.Error(), "toml: ")
	if m := tomlMismatch.FindStringSubmatch(msg); m != nil {
		msg = "unexpected TOML " + m[1]
		if len(key) >= 2 {
			switch key[len(key)-1] {
			case "harness", "test", "bench", "proc-macro":
				msg = "must be true or false"
			case "name", "path", "workspace", "version", "edition":
				msg = "must be a string"
			case "crate-type", "required-features":
				msg = "must be a list of strings"
			}
		}
	}
	if len(key) == 0 {
		return fmt.Errorf("rust: %s:%d: %s", manifestPath, row, msg)
	}
	return fmt.Errorf("rust: %s:%d: %s: %s", manifestPath, row, tomlKeyPath(key...), msg)
}

var tomlBareKey = regexp.MustCompile(`^[A-Za-z0-9_-]+$`)

// tomlKeyPath renders key parts as a TOML dotted key, quoting where needed.
func tomlKeyPath(parts ...string) string {
	out := make([]string, len(parts))
	for i, part := range parts {
		switch {
		case tomlBareKey.MatchString(part):
			out[i] = part
		case !strings.ContainsAny(part, "'\n\r"):
			out[i] = "'" + part + "'"
		default:
			out[i] = strconv.Quote(part)
		}
	}
	return strings.Join(out, ".")
}

// cargoDepEntries normalizes every dependency table into
// table -> dependency key -> target cfg ("" when unconditional) -> entry.
func (doc *cargoTOML) cargoDepEntries(manifestPath string) (map[string]map[string]map[string]*cargoDepEntry, error) {
	out := make(map[string]map[string]map[string]*cargoDepEntry)
	add := func(table, cfg string, deps map[string]any, prefix ...string) error {
		keys := make([]string, 0, len(deps))
		for key := range deps {
			keys = append(keys, key)
		}
		sort.Strings(keys)
		for _, key := range keys {
			keyPath := tomlKeyPath(append(append([]string{}, prefix...), key)...)
			entry, err := cargoDepEntryFrom(manifestPath, key, keyPath, deps[key])
			if err != nil {
				return err
			}
			entry.cfg = cfg
			if out[table] == nil {
				out[table] = make(map[string]map[string]*cargoDepEntry)
			}
			if out[table][key] == nil {
				out[table][key] = make(map[string]*cargoDepEntry)
			}
			out[table][key][cfg] = entry
		}
		return nil
	}
	plain := []struct {
		table string
		deps  func(p cargoTOMLPlatform) map[string]any
	}{
		{"dependencies", func(p cargoTOMLPlatform) map[string]any { return p.Dependencies }},
		{"dev-dependencies", func(p cargoTOMLPlatform) map[string]any { return p.DevDependencies }},
		{"build-dependencies", func(p cargoTOMLPlatform) map[string]any { return p.BuildDependencies }},
	}
	top := cargoTOMLPlatform{Dependencies: doc.Dependencies, DevDependencies: doc.DevDependencies, BuildDependencies: doc.BuildDependencies}
	for _, t := range plain {
		if err := add(t.table, "", t.deps(top), t.table); err != nil {
			return nil, err
		}
	}
	cfgs := make([]string, 0, len(doc.Target))
	for cfg := range doc.Target {
		cfgs = append(cfgs, cfg)
	}
	sort.Strings(cfgs)
	for _, cfg := range cfgs {
		for _, t := range plain {
			if err := add(t.table, cfg, t.deps(doc.Target[cfg]), "target", cfg, t.table); err != nil {
				return nil, err
			}
		}
	}
	if doc.Workspace != nil {
		if err := add("workspace", "", doc.Workspace.Dependencies, "workspace", "dependencies"); err != nil {
			return nil, err
		}
	}
	return out, nil
}

func cargoDepEntryFrom(manifestPath, key, keyPath string, value any) (*cargoDepEntry, error) {
	entry := &cargoDepEntry{key: key, keyPath: keyPath}
	switch v := value.(type) {
	case string:
		entry.version = v
		return entry, nil
	case map[string]any:
		str := func(field string) (*string, error) {
			raw, ok := v[field]
			if !ok {
				return nil, nil
			}
			s, ok := raw.(string)
			if !ok {
				return nil, fmt.Errorf("rust: %s: %s.%s must be a string", manifestPath, keyPath, field)
			}
			return &s, nil
		}
		version, err := str("version")
		if err != nil {
			return nil, err
		}
		if version != nil {
			entry.version = *version
		}
		if entry.depPath, err = str("path"); err != nil {
			return nil, err
		}
		if entry.pkg, err = str("package"); err != nil {
			return nil, err
		}
		if raw, ok := v["workspace"]; ok {
			b, ok := raw.(bool)
			if !ok {
				return nil, fmt.Errorf("rust: %s: %s.workspace must be true or false", manifestPath, keyPath)
			}
			entry.workspace = &b
		}
		return entry, nil
	default:
		return nil, fmt.Errorf("rust: %s: %s must be a version string or a table", manifestPath, keyPath)
	}
}
