package rust

import (
	"fmt"
	"os"
	"path"
	"sort"
	"strconv"
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"
)

const (
	exampleKind = "example"
	benchKind   = "bench"
)

type cargoTarget struct {
	kind             string // name is the Bazel target name, disambiguated when a library shares
	name             string
	logical          string
	crateName        string
	path             string
	harness          bool
	exampleTest      bool
	requiredFeatures []string
	crateTypes       []string
	procMacro        bool
	flavor           string
}

func (t cargoTarget) crate() string {
	if t.crateName != "" {
		return t.crateName
	}
	return t.name
}

type cargoManifest struct {
	packageName string
	edition     string
	version     string
	targets     []cargoTarget
	normalDeps  map[string]cargoDependency
	devDeps     map[string]cargoDependency
	buildDeps   map[string]cargoDependency
	virtual       bool
	workspaceRoot bool
	workspaceDeps map[string]cargoDependency
	build         *cargoBuild
}

type cargoBuild struct {
	path     string
	disabled bool
}

func (m *cargoManifest) withImplicitTargets(files map[string]bool, packagePath string) error {
	explicitPaths := make(map[string]bool)
	hasLib := false
	hasDefaultBin := false
	for _, target := range m.targets {
		explicitPaths[target.path] = true
		if target.kind == libraryKind {
			hasLib = true
		}
		if target.kind == binaryKind && target.path == "src/main.rs" {
			hasDefaultBin = true
		}
	}
	implicitLib := !hasLib && files[path.Join(packagePath, "src/lib.rs")]
	implicitBin := !hasDefaultBin && !explicitPaths["src/main.rs"] && files[path.Join(packagePath, "src/main.rs")]
	if implicitLib || implicitBin {
		name, err := common.Normalize(m.packageName)
		if err != nil {
			return fmt.Errorf("rust: [package].name %q: %w", m.packageName, err)
		}
		if implicitLib {
			m.targets = append(m.targets, cargoTarget{kind: libraryKind, name: name, logical: m.packageName, path: "src/lib.rs", harness: true})
		}
		if implicitBin {
			m.targets = append(m.targets, cargoTarget{kind: binaryKind, name: name, logical: m.packageName, path: "src/main.rs", harness: true})
		}
	}
	prefix := path.Join(packagePath, "tests") + "/"
	for file := range files {
		rel := strings.TrimPrefix(file, prefix)
		if !strings.HasPrefix(file, prefix) || rel == "" || strings.Contains(rel, "/") || !strings.HasSuffix(rel, ".rs") {
			continue
		}
		targetPath := "tests/" + rel
		if explicitPaths[targetPath] {
			continue
		}
		name, err := common.Normalize(strings.TrimSuffix(rel, ".rs"))
		if err != nil {
			return fmt.Errorf("rust: Cargo integration-test root %s: %w", file, err)
		}
		m.targets = append(m.targets, cargoTarget{kind: testKind, name: IntegrationTestName(name), logical: IntegrationTestName(name), path: targetPath, harness: true})
	}
	if m.build == nil && files[path.Join(packagePath, "build.rs")] {
		return fmt.Errorf("rust: %s/build.rs exists without a [package] build key; declare build = \"build.rs\" or build = false", packagePath)
	}
	m.disambiguateLibBin()
	return m.validateTargetClaims()
}

func (m *cargoManifest) disambiguateLibBin() {
	bins := make(map[string]bool)
	for _, target := range m.targets {
		if target.kind == binaryKind || target.kind == exampleKind || target.kind == benchKind {
			bins[target.name] = true
		}
	}
	for i := range m.targets {
		target := &m.targets[i]
		if target.kind == libraryKind && bins[target.name] {
			if target.crateName == "" {
				target.crateName = target.name
			}
			target.name += "_lib"
		}
	}
}

func (m *cargoManifest) validateTargetClaims() error {
	seenNames := make(map[string]string)
	seenPaths := make(map[string]string)
	for _, target := range m.targets {
		if len(target.requiredFeatures) > 0 {
			return fmt.Errorf("Cargo target %s(%s) requires features %q; required-features stay opt-in: keep a handwritten testonly target selecting them", target.logical, target.kind, strings.Join(target.requiredFeatures, ", "))
		}
		if target.kind == benchKind && target.harness {
			return fmt.Errorf("Cargo bench %s leaves the libtest harness enabled; benchmarks generate only with harness = false, otherwise keep a handwritten target", target.logical)
		}
		if previous, ok := seenNames[target.name]; ok {
			return fmt.Errorf("Cargo target name %q is claimed by %s and %s; use distinct authoritative names", target.name, previous, target.kind)
		}
		if previous, ok := seenPaths[target.path]; ok {
			return fmt.Errorf("Cargo target path %q is claimed by %s and %s; one source may have only one owner", target.path, previous, target.name)
		}
		seenNames[target.name] = target.kind
		seenPaths[target.path] = target.name
	}
	return nil
}

type cargoDependency struct {
	external  bool
	label     string
	version   string
	depPath   string
	workspace bool
}

type cargoDepField struct {
	value string
	line  int
}

type cargoDepEntry struct {
	key    string
	target bool
	fields map[string]cargoDepField
}

func parseCargoManifest(manifestPath string, content []byte) (*cargoManifest, error) {
	manifest := &cargoManifest{
		edition:    "2021",
		normalDeps:    make(map[string]cargoDependency),
		devDeps:       make(map[string]cargoDependency),
		buildDeps:     make(map[string]cargoDependency),
		workspaceDeps: make(map[string]cargoDependency),
	}
	section := ""
	depTable, depName := "", ""
	depTarget := false
	depFields := make(map[string]map[string]*cargoDepEntry)
	addDepField := func(table, key string, target bool, field, value string, line int) {
		entries := depFields[table]
		if entries == nil {
			entries = make(map[string]*cargoDepEntry)
			depFields[table] = entries
		}
		entry := entries[key]
		if entry != nil && !entry.target && target {
			return
		}
		if entry == nil || (entry.target && !target) {
			entry = &cargoDepEntry{key: key, target: target, fields: make(map[string]cargoDepField)}
			entries[key] = entry
		}
		entry.fields[field] = cargoDepField{value: value, line: line}
	}
	current := -1
	hasPackage := false
	hasWorkspace := false
	type logicalLine struct {
		number int
		text   string
	}
	var logical []logicalLine
	var pending strings.Builder
	pendingNumber := 0
	depth := 0
	var quote byte
	flush := func(number int) {
		if strings.TrimSpace(pending.String()) != "" {
			logical = append(logical, logicalLine{number: pendingNumber, text: pending.String()})
		}
		pending.Reset()
		depth = 0
		quote = 0
		_ = number
	}
	for number, raw := range strings.Split(string(content), "\n") {
		stripped := strings.TrimSpace(stripTomlComment(raw))
		if stripped == "" && pending.Len() == 0 {
			continue
		}
		if pending.Len() == 0 {
			pendingNumber = number + 1
		} else {
			pending.WriteString(" ")
		}
		pending.WriteString(stripped)
		for i := 0; i < len(stripped); i++ {
			c := stripped[i]
			if quote != 0 {
				if c == '\\' && quote == '"' {
					i++
				} else if c == quote {
					quote = 0
				}
				continue
			}
			switch c {
			case '"', '\'':
				quote = c
			case '[', '{':
				depth++
			case ']', '}':
				depth--
			}
		}
		if depth <= 0 {
			depth = 0
			flush(number + 1)
		}
	}
	if strings.TrimSpace(pending.String()) != "" {
		logical = append(logical, logicalLine{number: pendingNumber, text: pending.String()})
	}
	for _, ll := range logical {
		number := ll.number - 1
		line := strings.TrimSpace(ll.text)
		if line == "" {
			continue
		}
		if strings.HasPrefix(line, "[") {
			depTable = ""
			switch line {
			case "[package]", "[lib]":
				section = strings.Trim(line, "[]")
				if section == "package" {
					hasPackage = true
				}
				if section == "lib" {
					manifest.targets = append(manifest.targets, cargoTarget{kind: libraryKind, harness: true})
					current = len(manifest.targets) - 1
				} else {
					current = -1
				}
			case "[[bin]]", "[[test]]", "[[example]]", "[[bench]]":
				section = strings.Trim(line, "[]")
				kind := binaryKind
				switch section {
				case "test":
					kind = testKind
				case "example":
					kind = exampleKind
				case "bench":
					kind = benchKind
				}
				manifest.targets = append(manifest.targets, cargoTarget{kind: kind, harness: true})
				current = len(manifest.targets) - 1
			default:
				section = strings.Trim(line, "[]")
				if section == "workspace" || strings.HasPrefix(section, "workspace.") {
					hasWorkspace = true
				}
				current = -1
				header, err := splitTomlDottedKey(strings.TrimSuffix(strings.TrimPrefix(line, "["), "]"))
				if err != nil {
					return nil, fmt.Errorf("rust: %s:%d: malformed Cargo table header: %v", manifestPath, number+1, err)
				}
				if table, name, target, ok := cargoDepSection(header); ok {
					depTable, depName, depTarget = table, name, target
				}
			}
			continue
		}
		parts := strings.SplitN(line, "=", 2)
		if len(parts) != 2 {
			return nil, fmt.Errorf("rust: %s:%d: malformed Cargo assignment", manifestPath, number+1)
		}
		key, value := strings.TrimSpace(parts[0]), strings.TrimSpace(parts[1])
		if depTable != "" {
			keyParts, err := splitTomlDottedKey(key)
			if err != nil {
				return nil, fmt.Errorf("rust: %s:%d: malformed dependency key: %v", manifestPath, number+1, err)
			}
			if depName != "" {
				addDepField(depTable, depName, depTarget, strings.Join(keyParts, "."), value, number+1)
				continue
			}
			name := keyParts[0]
			if len(keyParts) > 1 {
				addDepField(depTable, name, depTarget, strings.Join(keyParts[1:], "."), value, number+1)
				continue
			}
			switch {
			case strings.HasPrefix(value, "{"):
				inline, err := tomlInlineTable(value)
				if err != nil {
					return nil, fmt.Errorf("rust: %s:%d: dependency %q: %v", manifestPath, number+1, name, err)
				}
				for field, fieldValue := range inline {
					addDepField(depTable, name, depTarget, field, fieldValue, number+1)
				}
			case strings.HasPrefix(value, "\""):
				addDepField(depTable, name, depTarget, "version", value, number+1)
			default:
				return nil, fmt.Errorf("rust: %s:%d: dependency %q must be a version string or a table", manifestPath, number+1, name)
			}
			continue
		}
		switch section {
		case "package":
			switch key {
			case "name", "edition":
				parsed, err := tomlString(value)
				if err != nil {
					return nil, fmt.Errorf("rust: %s:%d: %s: %v", manifestPath, number+1, key, err)
				}
				if key == "name" {
					manifest.packageName = parsed
				} else {
					manifest.edition = parsed
				}
			case "version":
				parsed, err := tomlString(value)
				if err != nil {
					return nil, fmt.Errorf("rust: %s:%d: %s: %v", manifestPath, number+1, key, err)
				}
				manifest.version = parsed
			case "build":
				switch value {
				case "false":
					manifest.build = &cargoBuild{disabled: true}
				case "true":
					manifest.build = &cargoBuild{path: "build.rs"}
				default:
					parsed, err := tomlString(value)
					if err != nil {
						return nil, fmt.Errorf("rust: %s:%d: build must be a path, true, or false", manifestPath, number+1)
					}
					manifest.build = &cargoBuild{path: path.Clean(parsed)}
				}
			}
		case "lib", "bin", "test", "example", "bench":
			switch key {
			case "name", "path":
				parsed, err := tomlString(value)
				if err != nil {
					return nil, fmt.Errorf("rust: %s:%d: %s: %v", manifestPath, number+1, key, err)
				}
				if key == "name" {
					manifest.targets[current].name = parsed
				} else {
					manifest.targets[current].path = path.Clean(parsed)
				}
			case "harness":
				parsed, err := strconv.ParseBool(value)
				if err != nil {
					return nil, fmt.Errorf("rust: %s:%d: harness must be true or false", manifestPath, number+1)
				}
				if section == "example" {
					return nil, fmt.Errorf("rust: %s:%d: [[example]] has no harness key; declare test = true for a libtest wrapper or keep the example handwritten", manifestPath, number+1)
				}
				manifest.targets[current].harness = parsed
			case "test":
				if section == "example" {
					parsed, err := strconv.ParseBool(value)
					if err != nil {
						return nil, fmt.Errorf("rust: %s:%d: test must be true or false", manifestPath, number+1)
					}
					manifest.targets[current].exampleTest = parsed
					break
				}
				if _, err := strconv.ParseBool(value); err != nil {
					return nil, fmt.Errorf("rust: %s:%d: test must be true or false", manifestPath, number+1)
				}
			case "bench":
				if _, err := strconv.ParseBool(value); err != nil {
					return nil, fmt.Errorf("rust: %s:%d: bench must be true or false", manifestPath, number+1)
				}
			case "crate-type":
				if section != "lib" {
					return nil, fmt.Errorf("rust: %s:%d: unsupported Cargo target field %q; use a kept handwritten target", manifestPath, number+1, key)
				}
				parsed, err := tomlStringList(value)
				if err != nil {
					return nil, fmt.Errorf("rust: %s:%d: crate-type: %v", manifestPath, number+1, err)
				}
				manifest.targets[current].crateTypes = parsed
			case "proc-macro":
				if section != "lib" {
					return nil, fmt.Errorf("rust: %s:%d: unsupported Cargo target field %q; use a kept handwritten target", manifestPath, number+1, key)
				}
				parsed, err := strconv.ParseBool(value)
				if err != nil {
					return nil, fmt.Errorf("rust: %s:%d: proc-macro must be true or false", manifestPath, number+1)
				}
				manifest.targets[current].procMacro = parsed
			case "required-features":
				parsed, err := tomlStringList(value)
				if err != nil {
					return nil, fmt.Errorf("rust: %s:%d: required-features: %v", manifestPath, number+1, err)
				}
				manifest.targets[current].requiredFeatures = parsed
			}
		}
	}
	if err := manifest.buildDependencies(manifestPath, depFields); err != nil {
		return nil, err
	}
	manifest.workspaceRoot = hasWorkspace
	if manifest.packageName == "" {
		if hasWorkspace && !hasPackage {
			manifest.virtual = true
			return manifest, nil
		}
		return nil, fmt.Errorf("rust: %s: missing [package].name", manifestPath)
	}
	if _, err := common.Normalize(manifest.packageName); err != nil {
		return nil, fmt.Errorf("rust: %s: [package].name %q: %v", manifestPath, manifest.packageName, err)
	}
	for i := range manifest.targets {
		target := &manifest.targets[i]
		if target.name == "" && target.path == "" && (target.kind == exampleKind || target.kind == benchKind) {
			return nil, fmt.Errorf("rust: %s: %s target needs a name or path", manifestPath, target.kind)
		}
		if target.name == "" {
			if target.path != "" && (target.kind == exampleKind || target.kind == benchKind) {
				target.name = strings.TrimSuffix(path.Base(target.path), ".rs")
			} else {
				target.name = manifest.packageName
			}
		}
		if target.path == "" {
			switch target.kind {
			case libraryKind:
				target.path = "src/lib.rs"
			case binaryKind:
				target.path = "src/main.rs"
			case testKind:
				target.path = "tests/" + target.name + ".rs"
			case exampleKind:
				target.path = "examples/" + target.name + ".rs"
			case benchKind:
				target.path = "benches/" + target.name + ".rs"
			}
		}
		target.logical = target.name
		normalized, err := common.Normalize(target.name)
		if err != nil {
			return nil, fmt.Errorf("rust: %s: Cargo target %q: %v", manifestPath, target.name, err)
		}
		switch target.kind {
		case exampleKind, benchKind:
			target.name = normalized
			if target.kind == exampleKind {
				target.name += "_example"
			} else {
				target.name += "_bench"
			}
			target.crateName = normalized
		default:
			target.name = normalized
		}
	}
	if err := manifest.resolveLibFlavors(); err != nil {
		return nil, fmt.Errorf("rust: %s: %v", manifestPath, err)
	}
	manifest.disambiguateLibBin()
	if err := manifest.validateTargetClaims(); err != nil {
		return nil, fmt.Errorf("rust: %s: %v", manifestPath, err)
	}
	return manifest, nil
}

func (m *cargoManifest) resolveLibFlavors() error {
	for i := range m.targets {
		target := &m.targets[i]
		if target.kind != libraryKind {
			continue
		}
		types := target.crateTypes
		conflict := fmt.Errorf("library %s sets proc-macro = true with crate-type = %q; declare one shape", target.logical, types)
		switch {
		case target.procMacro && len(types) == 0:
			target.flavor = "proc-macro"
		case len(types) == 0:
			target.flavor = ""
		case len(types) > 1:
			return fmt.Errorf("library %s declares multiple crate types %q; one source owner maps to one Bazel target, so keep a handwritten target", target.logical, types)
		default:
			switch types[0] {
			case "lib", "rlib":
				if target.procMacro {
					return conflict
				}
				target.flavor = ""
			case "proc-macro":
				target.flavor = "proc-macro"
			case "cdylib":
				if target.procMacro {
					return conflict
				}
				target.flavor = "cdylib"
			case "staticlib":
				if target.procMacro {
					return conflict
				}
				target.flavor = "staticlib"
			default:
				return fmt.Errorf("library %s declares unsupported crate-type %q; keep a handwritten target", target.logical, types[0])
			}
		}
	}
	return nil
}

func stripTomlComment(line string) string {
	var quote rune
	escaped := false
	for i, char := range line {
		if quote != 0 {
			if escaped {
				escaped = false
			} else if char == '\\' && quote == '"' {
				escaped = true
			} else if char == quote {
				quote = 0
			}
		} else if char == '"' || char == '\'' {
			quote = char
		} else if char == '#' {
			return line[:i]
		}
	}
	return line
}

func splitTomlDottedKey(s string) ([]string, error) {
	var raw []string
	start := 0
	for i := 0; i < len(s); i++ {
		switch s[i] {
		case '"':
			end := tomlBasicEnd(s, i)
			if end < 0 {
				return nil, fmt.Errorf("unterminated quoted key in %q", s)
			}
			i = end
		case '\'':
			end := strings.IndexByte(s[i+1:], '\'')
			if end < 0 {
				return nil, fmt.Errorf("unterminated quoted key in %q", s)
			}
			i += end + 1
		case '.':
			raw = append(raw, s[start:i])
			start = i + 1
		}
	}
	raw = append(raw, s[start:])
	parts := make([]string, 0, len(raw))
	for _, part := range raw {
		part = strings.TrimSpace(part)
		switch {
		case part == "":
			return nil, fmt.Errorf("empty key part in %q", s)
		case part[0] == '"':
			unquoted, err := strconv.Unquote(part)
			if err != nil {
				return nil, fmt.Errorf("invalid quoted key %s", part)
			}
			part = unquoted
		case part[0] == '\'':
			if len(part) < 2 || part[len(part)-1] != '\'' {
				return nil, fmt.Errorf("invalid quoted key %s", part)
			}
			part = part[1 : len(part)-1]
		}
		parts = append(parts, part)
	}
	return parts, nil
}

func tomlBasicEnd(s string, open int) int {
	for i := open + 1; i < len(s); i++ {
		switch s[i] {
		case '\\':
			i++
		case '"':
			return i
		}
	}
	return -1
}

func cargoDepSection(parts []string) (table, name string, target, ok bool) {
	if len(parts) >= 3 && parts[0] == "target" {
		target = true
		parts = parts[2:]
	} else if len(parts) >= 2 && parts[0] == "workspace" && parts[1] == "dependencies" {
		if len(parts) > 3 {
			return "", "", false, false
		}
		if len(parts) == 3 {
			name = parts[2]
		}
		return "workspace", name, false, true
	}
	if len(parts) > 2 {
		return "", "", false, false
	}
	switch parts[0] {
	case "dependencies", "dev-dependencies", "build-dependencies":
		table = parts[0]
	default:
		return "", "", false, false
	}
	if len(parts) == 2 {
		name = parts[1]
	}
	return table, name, target, true
}

func tomlInlineTable(value string) (map[string]string, error) {
	value = strings.TrimSpace(value)
	if len(value) < 2 || value[0] != '{' || value[len(value)-1] != '}' {
		return nil, fmt.Errorf("expected an inline table")
	}
	inner := strings.TrimSpace(value[1 : len(value)-1])
	out := make(map[string]string)
	if inner == "" {
		return out, nil
	}
	var items []string
	depth, start := 0, 0
	for i := 0; i < len(inner); i++ {
		switch inner[i] {
		case '"':
			end := tomlBasicEnd(inner, i)
			if end < 0 {
				return nil, fmt.Errorf("unterminated string in inline table")
			}
			i = end
		case '\'':
			end := strings.IndexByte(inner[i+1:], '\'')
			if end < 0 {
				return nil, fmt.Errorf("unterminated string in inline table")
			}
			i += end + 1
		case '[', '{':
			depth++
		case ']', '}':
			depth--
		case ',':
			if depth == 0 {
				items = append(items, inner[start:i])
				start = i + 1
			}
		}
	}
	if depth != 0 {
		return nil, fmt.Errorf("unbalanced brackets in inline table")
	}
	items = append(items, inner[start:])
	for _, item := range items {
		eq := tomlKeyEnd(item)
		if eq < 0 {
			return nil, fmt.Errorf("inline table item %q needs key = value", strings.TrimSpace(item))
		}
		keyParts, err := splitTomlDottedKey(item[:eq])
		if err != nil {
			return nil, err
		}
		fieldValue := strings.TrimSpace(item[eq+1:])
		if fieldValue == "" {
			return nil, fmt.Errorf("inline table key %q has no value", strings.Join(keyParts, "."))
		}
		out[strings.Join(keyParts, ".")] = fieldValue
	}
	return out, nil
}

func tomlKeyEnd(item string) int {
	for i := 0; i < len(item); i++ {
		switch item[i] {
		case '"':
			i = tomlBasicEnd(item, i)
		case '\'':
			i += strings.IndexByte(item[i+1:], '\'') + 1
		case '=':
			return i
		}
	}
	return -1
}

func (m *cargoManifest) buildDependencies(manifestPath string, depFields map[string]map[string]*cargoDepEntry) error {
	targets := map[string]map[string]cargoDependency{
		"dependencies":       m.normalDeps,
		"dev-dependencies":   m.devDeps,
		"build-dependencies": m.buildDeps,
		"workspace":          m.workspaceDeps,
	}
	for table, entries := range depFields {
		keys := make([]string, 0, len(entries))
		for key := range entries {
			keys = append(keys, key)
		}
		sort.Strings(keys)
		for _, key := range keys {
			entry := entries[key]
			dep := cargoDependency{external: true, label: key}
			str := func(field string) (string, bool, error) {
				f, ok := entry.fields[field]
				if !ok {
					return "", false, nil
				}
				parsed, err := tomlString(f.value)
				if err != nil {
					return "", false, fmt.Errorf("rust: %s:%d: dependency %q %s: %v", manifestPath, f.line, key, field, err)
				}
				return parsed, true, nil
			}
			version, _, err := str("version")
			if err != nil {
				return err
			}
			dep.version = version
			depPath, hasPath, err := str("path")
			if err != nil {
				return err
			}
			if hasPath {
				dep.external = false
				dep.depPath = depPath
			}
			pkg, hasPkg, err := str("package")
			if err != nil {
				return err
			}
			if hasPkg {
				dep.label = pkg
			}
			if f, ok := entry.fields["workspace"]; ok {
				parsed, err := strconv.ParseBool(f.value)
				if err != nil {
					return fmt.Errorf("rust: %s:%d: dependency %q workspace must be true or false", manifestPath, f.line, key)
				}
				if parsed && hasPkg {
					return fmt.Errorf("rust: %s:%d: dependency %q sets both workspace = true and package; set package in [workspace.dependencies]", manifestPath, entry.fields["package"].line, key)
				}
				dep.workspace = parsed
			}
			targets[table][strings.ReplaceAll(key, "-", "_")] = dep
		}
	}
	return nil
}

func (m *cargoManifest) workspaceDep() (string, bool) {
	var names []string
	for _, depMap := range []map[string]cargoDependency{m.normalDeps, m.devDeps, m.buildDeps} {
		for _, dep := range depMap {
			if dep.workspace {
				names = append(names, dep.label)
			}
		}
	}
	if len(names) == 0 {
		return "", false
	}
	sort.Strings(names)
	return names[0], true
}

func (m *cargoManifest) inheritWorkspaceDeps(memberRel, rootRel string, root *cargoManifest) error {
	for _, depMap := range []map[string]cargoDependency{m.normalDeps, m.devDeps, m.buildDeps} {
		keys := make([]string, 0, len(depMap))
		for key := range depMap {
			keys = append(keys, key)
		}
		sort.Strings(keys)
		for _, key := range keys {
			dep := depMap[key]
			if !dep.workspace {
				continue
			}
			inherited, ok := root.workspaceDeps[key]
			if !ok {
				return fmt.Errorf("dependency %q sets workspace = true but %s has no [workspace.dependencies] entry", dep.label, path.Join(rootRel, "Cargo.toml"))
			}
			dep.external = inherited.external
			dep.label = inherited.label
			dep.version = inherited.version
			dep.depPath = ""
			if !inherited.external {
				dep.depPath = relSlashPath(memberRel, path.Join(rootRel, inherited.depPath))
			}
			depMap[key] = dep
		}
	}
	return nil
}

func relSlashPath(from, to string) string {
	split := func(p string) []string {
		p = path.Clean(p)
		if p == "." || p == "" {
			return nil
		}
		return strings.Split(p, "/")
	}
	fromParts, toParts := split(from), split(to)
	shared := 0
	for shared < len(fromParts) && shared < len(toParts) && fromParts[shared] == toParts[shared] {
		shared++
	}
	var out []string
	for range fromParts[shared:] {
		out = append(out, "..")
	}
	out = append(out, toParts[shared:]...)
	if len(out) == 0 {
		return "."
	}
	return strings.Join(out, "/")
}

func tomlString(value string) (string, error) {
	if len(value) < 2 || value[0] != '"' || value[len(value)-1] != '"' {
		return "", fmt.Errorf("expected a basic quoted string")
	}
	parsed, err := strconv.Unquote(value)
	if err != nil {
		return "", err
	}
	return parsed, nil
}

func tomlStringList(value string) ([]string, error) {
	trimmed := strings.TrimSpace(value)
	if len(trimmed) < 2 || trimmed[0] != '[' || trimmed[len(trimmed)-1] != ']' {
		return nil, fmt.Errorf("expected a string list")
	}
	inner := strings.TrimSpace(trimmed[1 : len(trimmed)-1])
	if inner == "" {
		return nil, nil
	}
	var out []string
	for _, item := range strings.Split(inner, ",") {
		parsed, err := tomlString(strings.TrimSpace(item))
		if err != nil {
			return nil, err
		}
		out = append(out, parsed)
	}
	return out, nil
}

func scanPackageVersion(content []byte) string {
	section := ""
	for _, raw := range strings.Split(string(content), "\n") {
		line := strings.TrimSpace(stripTomlComment(raw))
		if line == "" {
			continue
		}
		if strings.HasPrefix(line, "[") {
			section = strings.Trim(line, "[]")
			continue
		}
		if section != "package" {
			continue
		}
		parts := strings.SplitN(line, "=", 2)
		if len(parts) != 2 || strings.TrimSpace(parts[0]) != "version" {
			continue
		}
		if version, err := tomlString(strings.TrimSpace(parts[1])); err == nil {
			return version
		}
	}
	return ""
}

func checkPathDepVersions(packageDir, manifestPath string, manifest *cargoManifest) error {
	depMaps := []map[string]cargoDependency{manifest.normalDeps, manifest.devDeps, manifest.buildDeps}
	for _, depMap := range depMaps {
		for key, dep := range depMap {
			if dep.external || dep.version == "" || dep.depPath == "" {
				continue
			}
			provider := path.Join(packageDir, dep.depPath, "Cargo.toml")
			content, err := os.ReadFile(provider)
			if err != nil {
				return fmt.Errorf("path dependency %q points at %s without a readable Cargo.toml; fix the path", key, provider)
			}
			provided := scanPackageVersion(content)
			if provided == "" {
				return fmt.Errorf("path dependency %q at %s declares no [package] version while %s requires %q; version the provider or drop the requirement", key, provider, manifestPath, dep.version)
			}
			if !versionReqSatisfied(provided, dep.version) {
				return fmt.Errorf("path dependency %q provides version %s at %s, which does not satisfy %q required by %s", key, provided, provider, dep.version, manifestPath)
			}
		}
	}
	return nil
}

func versionReqSatisfied(version, req string) bool {
	req = strings.TrimSpace(req)
	if req == "" || req == "*" {
		return true
	}
	for _, clause := range strings.Split(req, ",") {
		if !evalVersionClause(version, strings.TrimSpace(clause)) {
			return false
		}
	}
	return true
}

func evalVersionClause(version, clause string) bool {
	bound := ""
	rest := clause
	for _, op := range []string{">=", "<=", ">", "<", "=", "^", "~"} {
		if strings.HasPrefix(clause, op) {
			bound = op
			rest = strings.TrimSpace(strings.TrimPrefix(clause, op))
			break
		}
	}
	current := parseSemver(version)
	want := parseSemver(rest)
	cmp := compareSemver(current, want)
	switch bound {
	case ">=":
		return cmp >= 0
	case "<=":
		return cmp <= 0
	case ">":
		return cmp > 0
	case "<":
		return cmp < 0
	case "=":
		return cmp == 0
	case "~":
		return cmp >= 0 && compareSemver(current, tildeUpper(want, rest)) < 0
	default:
		return cmp >= 0 && compareSemver(current, caretUpper(want, rest)) < 0
	}
}

func parseSemver(version string) [3]int {
	var out [3]int
	core := version
	if idx := strings.IndexAny(core, "-+"); idx >= 0 {
		core = core[:idx]
	}
	for i, part := range strings.Split(core, ".") {
		if i >= 3 {
			break
		}
		number, err := strconv.Atoi(strings.TrimSpace(part))
		if err != nil {
			return [3]int{-1, -1, -1}
		}
		out[i] = number
	}
	return out
}

func compareSemver(a, b [3]int) int {
	for i := 0; i < 3; i++ {
		if a[i] != b[i] {
			if a[i] < b[i] {
				return -1
			}
			return 1
		}
	}
	return 0
}

func caretUpper(want [3]int, rest string) [3]int {
	parts := strings.Split(rest, ".")
	switch {
	case want[0] > 0:
		return [3]int{want[0] + 1, 0, 0}
	case len(parts) == 1:
		return [3]int{1, 0, 0}
	case want[1] > 0:
		return [3]int{0, want[1] + 1, 0}
	default:
		return [3]int{0, 0, want[2] + 1}
	}
}

func tildeUpper(want [3]int, rest string) [3]int {
	if len(strings.Split(rest, ".")) <= 1 {
		return [3]int{want[0] + 1, 0, 0}
	}
	return [3]int{want[0], want[1] + 1, 0}
}
