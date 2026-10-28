package rust

import (
	"fmt"
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

// libCrateName returns the extern crate name of the manifest's library, or ""
// when it has none. It does not mutate the manifest.
func (m *cargoManifest) libCrateName() string {
	for _, t := range m.targets {
		if t.kind == libraryKind {
			return t.crate()
		}
	}
	name, err := CrateName(m.packageName)
	if err != nil {
		return ""
	}
	return name
}

type cargoManifest struct {
	packageName   string
	edition       string
	version       string
	targets       []cargoTarget
	normalDeps    map[string]cargoDependency
	devDeps       map[string]cargoDependency
	buildDeps     map[string]cargoDependency
	virtual       bool
	workspaceRoot bool
	workspaceDeps map[string]cargoDependency
	build         *cargoBuild

	inheritVersion   bool
	inheritEdition   bool
	workspacePath    string
	workspaceVersion string
	workspaceEdition string
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
		crate, err := CrateName(m.packageName)
		if err != nil {
			return fmt.Errorf("rust: [package].name %q: %w", m.packageName, err)
		}
		if implicitLib {
			m.targets = append(m.targets, cargoTarget{kind: libraryKind, name: name, crateName: crate, logical: m.packageName, path: "src/lib.rs", harness: true})
		}
		if implicitBin {
			m.targets = append(m.targets, cargoTarget{kind: binaryKind, name: name, crateName: crate, logical: m.packageName, path: "src/main.rs", harness: true})
		}
	}
	prefix := path.Join(packagePath, "tests") + "/"
	var testRoots []string
	for file := range files {
		rel := strings.TrimPrefix(file, prefix)
		if !strings.HasPrefix(file, prefix) || rel == "" || strings.Contains(rel, "/") || !strings.HasSuffix(rel, ".rs") {
			continue
		}
		testRoots = append(testRoots, rel)
	}
	sort.Strings(testRoots)
	for _, rel := range testRoots {
		targetPath := "tests/" + rel
		if explicitPaths[targetPath] {
			continue
		}
		name, err := common.Normalize(strings.TrimSuffix(rel, ".rs"))
		if err != nil {
			return fmt.Errorf("rust: Cargo integration-test root %s: %w", prefix+rel, err)
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
	renamed   bool
	crate     string
	cfgs      string
}

const androidPlatformConstraint = "@platforms//os:android"

func cargoCfgPlatform(cfg string) (string, bool) {
	switch strings.Join(strings.Fields(cfg), "") {
	case `cfg(target_os="android")`:
		return androidPlatformConstraint, true
	case `cfg(target_os="windows")`, `cfg(windows)`:
		return "@platforms//os:windows", true
	case `cfg(target_os="linux")`:
		return "@platforms//os:linux", true
	case `cfg(target_os="macos")`:
		return "@platforms//os:macos", true
	case `cfg(target_os="ios")`:
		return "@platforms//os:ios", true
	}
	return "", false
}

// platformConstraint returns the single platform constraint every cfg of a
// local path dependency maps to, or "" when the dependency is not gated.
func (d cargoDependency) platformConstraint() string {
	if d.external || d.cfgs == "" {
		return ""
	}
	out := ""
	for _, cfg := range strings.Split(d.cfgs, "\n") {
		constraint, ok := cargoCfgPlatform(cfg)
		if !ok || (out != "" && constraint != out) {
			return ""
		}
		out = constraint
	}
	return out
}

func (d cargoDependency) androidOnly() bool {
	return d.platformConstraint() == androidPlatformConstraint
}

func parseCargoManifest(manifestPath string, content []byte) (*cargoManifest, error) {
	manifest := &cargoManifest{
		edition:       "2021",
		normalDeps:    make(map[string]cargoDependency),
		devDeps:       make(map[string]cargoDependency),
		buildDeps:     make(map[string]cargoDependency),
		workspaceDeps: make(map[string]cargoDependency),
	}
	doc, err := decodeCargoTOML(manifestPath, content)
	if err != nil {
		return nil, err
	}
	hasPackage := doc.Package != nil
	hasWorkspace := doc.Workspace != nil
	if pkg := doc.Package; pkg != nil {
		manifest.packageName = pkg.Name
		if pkg.Workspace != "" {
			manifest.workspacePath = path.Clean(pkg.Workspace)
		}
		for _, field := range []struct {
			name    string
			value   any
			to      *string
			inherit *bool
		}{
			{"version", pkg.Version, &manifest.version, &manifest.inheritVersion},
			{"edition", pkg.Edition, &manifest.edition, &manifest.inheritEdition},
		} {
			switch v := field.value.(type) {
			case nil:
			case string:
				*field.to = v
			case map[string]any:
				if len(v) != 1 || v["workspace"] != true {
					return nil, fmt.Errorf("rust: %s: package.%s must be a string or { workspace = true }", manifestPath, field.name)
				}
				*field.inherit = true
			default:
				return nil, fmt.Errorf("rust: %s: package.%s must be a string or { workspace = true }", manifestPath, field.name)
			}
		}
		switch v := pkg.Build.(type) {
		case nil:
		case bool:
			if v {
				manifest.build = &cargoBuild{path: "build.rs"}
			} else {
				manifest.build = &cargoBuild{disabled: true}
			}
		case string:
			manifest.build = &cargoBuild{path: path.Clean(v)}
		default:
			return nil, fmt.Errorf("rust: %s: package.build must be a path, true, or false", manifestPath)
		}
	}
	if ws := doc.Workspace; ws != nil {
		manifest.workspaceVersion = ws.Package.Version
		manifest.workspaceEdition = ws.Package.Edition
	}
	if doc.Lib != nil {
		target, err := cargoTargetFrom(manifestPath, libraryKind, *doc.Lib)
		if err != nil {
			return nil, err
		}
		manifest.targets = append(manifest.targets, target)
	}
	for _, group := range []struct {
		kind    string
		targets []cargoTOMLTarget
	}{
		{binaryKind, doc.Bin},
		{testKind, doc.Test},
		{exampleKind, doc.Example},
		{benchKind, doc.Bench},
	} {
		for _, raw := range group.targets {
			target, err := cargoTargetFrom(manifestPath, group.kind, raw)
			if err != nil {
				return nil, err
			}
			manifest.targets = append(manifest.targets, target)
		}
	}
	depFields, err := doc.cargoDepEntries(manifestPath)
	if err != nil {
		return nil, err
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
	if _, err := CrateName(manifest.packageName); err != nil {
		return nil, fmt.Errorf("rust: %s: [package].name %q: %v", manifestPath, manifest.packageName, err)
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
		crate, err := CrateName(target.logical)
		if err != nil {
			return nil, fmt.Errorf("rust: %s: Cargo target %q: %v", manifestPath, target.logical, err)
		}
		target.crateName = crate
		switch target.kind {
		case exampleKind, benchKind:
			target.name = normalized
			if target.kind == exampleKind {
				target.name += "_example"
			} else {
				target.name += "_bench"
			}
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

func cargoTargetFrom(manifestPath, kind string, raw cargoTOMLTarget) (cargoTarget, error) {
	target := cargoTarget{kind: kind, name: raw.Name, harness: true}
	table := "[[" + kind + "]]"
	if kind == libraryKind {
		table = "[lib]"
	}
	if raw.Path != nil {
		target.path = path.Clean(*raw.Path)
	}
	if raw.Harness != nil {
		if kind == exampleKind {
			return target, fmt.Errorf("rust: %s: [[example]] has no harness key; declare test = true for a libtest wrapper or keep the example handwritten", manifestPath)
		}
		target.harness = *raw.Harness
	}
	if kind == exampleKind && raw.Test != nil {
		target.exampleTest = *raw.Test
	}
	if kind != libraryKind {
		for field, set := range map[string]bool{"crate-type": raw.CrateType != nil, "proc-macro": raw.ProcMacro != nil} {
			if set {
				return target, fmt.Errorf("rust: %s: %s: unsupported Cargo target field %q; use a kept handwritten target", manifestPath, table, field)
			}
		}
	}
	if raw.CrateType != nil {
		target.crateTypes = *raw.CrateType
	}
	if raw.ProcMacro != nil {
		target.procMacro = *raw.ProcMacro
	}
	if raw.RequiredFeatures != nil {
		target.requiredFeatures = *raw.RequiredFeatures
	}
	return target, nil
}

func (m *cargoManifest) buildDependencies(manifestPath string, depFields map[string]map[string]map[string]*cargoDepEntry) error {
	targets := map[string]map[string]cargoDependency{
		"dependencies":       m.normalDeps,
		"dev-dependencies":   m.devDeps,
		"build-dependencies": m.buildDeps,
		"workspace":          m.workspaceDeps,
	}
	for _, table := range cargoDepTables {
		entries := depFields[table]
		keys := make([]string, 0, len(entries))
		for key := range entries {
			keys = append(keys, key)
		}
		sort.Strings(keys)
		for _, key := range keys {
			variants := entries[key]
			cfgs := make([]string, 0, len(variants))
			for cfg := range variants {
				cfgs = append(cfgs, cfg)
			}
			sort.Strings(cfgs)
			var chosen cargoDependency
			var first *cargoDepEntry
			for i, cfg := range cfgs {
				entry := variants[cfg]
				dep, err := buildCargoDependency(manifestPath, entry)
				if err != nil {
					return err
				}
				if i == 0 {
					chosen, first = dep, entry
					if cfg != "" && !dep.external {
						chosen.cfgs = cfg
					}
					continue
				}
				if first.cfg == "" {
					continue
				}
				if !dep.external {
					chosen.cfgs += "\n" + cfg
				}
				same := dep.external == chosen.external && dep.label == chosen.label && dep.depPath == chosen.depPath && dep.workspace == chosen.workspace && dep.renamed == chosen.renamed
				if same && !dep.external && dep.version != chosen.version {
					same = false
				}
				if !same {
					return fmt.Errorf("rust: %s: %s differs from %s; declare one form or keep a handwritten target", manifestPath, entry.keyPath, first.keyPath)
				}
			}
			crate, err := CrateName(key)
			if err != nil {
				return fmt.Errorf("rust: %s: dependency %q: %v", manifestPath, key, err)
			}
			targets[table][crate] = chosen
		}
	}
	return nil
}

func buildCargoDependency(manifestPath string, entry *cargoDepEntry) (cargoDependency, error) {
	dep := cargoDependency{external: true, label: entry.key, version: entry.version}
	if entry.depPath != nil {
		dep.external = false
		dep.depPath = *entry.depPath
	}
	if entry.pkg != nil {
		dep.label = *entry.pkg
		dep.renamed = true
	}
	if entry.workspace != nil {
		if *entry.workspace && entry.pkg != nil {
			return dep, fmt.Errorf("rust: %s: %s sets both workspace = true and package; set package in [workspace.dependencies]", manifestPath, entry.keyPath)
		}
		dep.workspace = *entry.workspace
	}
	return dep, nil
}

func (m *cargoManifest) inheritsWorkspace() (string, bool) {
	var names []string
	for _, depMap := range []map[string]cargoDependency{m.normalDeps, m.devDeps, m.buildDeps} {
		for _, dep := range depMap {
			if dep.workspace {
				names = append(names, dep.label)
			}
		}
	}
	switch {
	case len(names) > 0:
		sort.Strings(names)
		return fmt.Sprintf("dependency %q", names[0]), true
	case m.inheritVersion:
		return "package.version", true
	case m.inheritEdition:
		return "package.edition", true
	}
	return "", false
}

func (m *cargoManifest) inheritWorkspacePackage(rootRel string, root *cargoManifest) error {
	for _, field := range []struct {
		name    string
		inherit bool
		from    string
		to      *string
	}{
		{"version", m.inheritVersion, root.workspaceVersion, &m.version},
		{"edition", m.inheritEdition, root.workspaceEdition, &m.edition},
	} {
		if !field.inherit {
			continue
		}
		if field.from == "" {
			return fmt.Errorf("package.%s sets workspace = true but %s has no [workspace.package] %s", field.name, path.Join(rootRel, "Cargo.toml"), field.name)
		}
		*field.to = field.from
	}
	return nil
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
			dep.renamed = inherited.renamed
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
