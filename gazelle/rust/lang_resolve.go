package rust

import (
	"fmt"
	"path"
	"sort"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/repo"
	"github.com/bazelbuild/bazel-gazelle/resolve"
	bzl "github.com/bazelbuild/buildtools/build"
	"github.com/devhusdk/rules_dx/gazelle/common"
)

const resolveExtName = "_resolve"

func lookupOverride(c *config.Config, name string) (label.Label, bool) {
	var zero label.Label
	if c == nil {
		return zero, false
	}
	if _, ok := c.Exts[resolveExtName]; !ok {
		return zero, false
	}
	return resolve.FindRuleWithOverride(c, resolve.ImportSpec{Lang: languageName, Imp: name}, languageName)
}

func resolveImportOverride(c *config.Config, name string) (bool, error) {
	if _, ok := lookupOverride(c, name); ok {
		if ignore := common.MatchingIgnore(c, languageName, name); ignore != nil {
			return false, fmt.Errorf("import %q has both an exact resolve mapping and an ignore directive", name)
		}
		return true, nil
	}
	return false, nil
}

func validateCargoImports(c *config.Config, manifest *cargoManifest, kind string, imports targetImports) error {
	for _, name := range imports.production {
		mapped, err := resolveImportOverride(c, name)
		if err != nil {
			return err
		}
		if mapped {
			continue
		}
		_, normal := manifest.normalDeps[name]
		_, dev := manifest.devDeps[name]
		if !normal && !(kind == testKind && dev) {
			self, err := CrateName(manifest.packageName)
			if err != nil {
				return err
			}
			if name == self {
				continue
			}
			return fmt.Errorf("unresolved production import %q; declare it in [dependencies] or add an exact mapping", name)
		}
	}
	for _, name := range imports.test {
		mapped, err := resolveImportOverride(c, name)
		if err != nil {
			return err
		}
		if mapped {
			continue
		}
		_, normal := manifest.normalDeps[name]
		_, dev := manifest.devDeps[name]
		if !normal && !dev {
			return fmt.Errorf("unresolved test import %q; declare it in [dev-dependencies] or add an exact mapping", name)
		}
	}
	return nil
}

func validateExampleImports(c *config.Config, manifest *cargoManifest, imports targetImports) error {
	for _, name := range imports.production {
		mapped, err := resolveImportOverride(c, name)
		if err != nil {
			return err
		}
		if mapped {
			continue
		}
		_, normal := manifest.normalDeps[name]
		_, dev := manifest.devDeps[name]
		if !normal && !dev {
			self, err := CrateName(manifest.packageName)
			if err != nil {
				return err
			}
			if name == self {
				continue
			}
			return fmt.Errorf("unresolved production import %q; declare it in [dependencies] or [dev-dependencies] or add an exact mapping", name)
		}
	}
	for _, name := range imports.test {
		mapped, err := resolveImportOverride(c, name)
		if err != nil {
			return err
		}
		if mapped {
			continue
		}
		_, normal := manifest.normalDeps[name]
		_, dev := manifest.devDeps[name]
		if !normal && !dev {
			return fmt.Errorf("unresolved test import %q; declare it in [dev-dependencies] or add an exact mapping", name)
		}
	}
	return nil
}

func validateBuildImports(c *config.Config, manifest *cargoManifest, imports targetImports) error {
	check := func(names []string, scope string) error {
		for _, name := range names {
			mapped, err := resolveImportOverride(c, name)
			if err != nil {
				return err
			}
			if mapped {
				continue
			}
			if _, ok := manifest.buildDeps[name]; !ok {
				return fmt.Errorf("unresolved %s import %q; declare it in [build-dependencies] or add an exact mapping", scope, name)
			}
		}
		return nil
	}
	if err := check(imports.production, "production"); err != nil {
		return err
	}
	return check(imports.test, "test")
}

func noteRename(result *targetImports, name string, dep cargoDependency) {
	if dep.crate != "" && dep.crate != name {
		if result.renames == nil {
			result.renames = make(map[string]string)
		}
		result.renames[name] = dep.crate
	}
}

func localCargoImports(c *config.Config, manifest *cargoManifest, imports targetImports, includeDev bool) targetImports {
	var result targetImports
	for _, name := range imports.production {
		if dep, ok := manifest.normalDeps[name]; ok && !dep.external {
			result.production = append(result.production, name)
			noteRename(&result, name, dep)
		} else if includeDev {
			if dep, ok := manifest.devDeps[name]; ok && !dep.external {
				result.production = append(result.production, name)
				noteRename(&result, name, dep)
			} else if _, ok := lookupOverride(c, name); ok {
				result.production = append(result.production, name)
			}
		} else if _, ok := lookupOverride(c, name); ok {
			result.production = append(result.production, name)
		}
	}
	if includeDev {
		for _, name := range imports.test {
			if dep, normal := manifest.normalDeps[name]; normal && !dep.external {
				result.test = append(result.test, name)
				noteRename(&result, name, dep)
			} else if dep, dev := manifest.devDeps[name]; dev && !dep.external {
				result.test = append(result.test, name)
				noteRename(&result, name, dep)
			} else if _, ok := lookupOverride(c, name); ok {
				result.test = append(result.test, name)
			}
		}
	}
	appendMirrorPaths(&result, manifest.normalDeps)
	if includeDev {
		appendMirrorPaths(&result, manifest.devDeps)
	}
	return result
}

func appendMirrorPaths(result *targetImports, scopes ...map[string]cargoDependency) {
	seen := make(map[string]bool, len(result.mirrorPaths))
	for _, name := range result.mirrorPaths {
		seen[name] = true
	}
	for _, scope := range scopes {
		for name, dep := range scope {
			if !dep.external && dep.depPath != "" && !seen[name] {
				seen[name] = true
				result.mirrorPaths = append(result.mirrorPaths, name)
				noteRename(result, name, dep)
			}
		}
	}
	sort.Strings(result.mirrorPaths)
}

func localCargoExampleImports(c *config.Config, manifest *cargoManifest, imports targetImports) targetImports {
	var result targetImports
	for _, name := range imports.production {
		if dep, ok := manifest.normalDeps[name]; ok && !dep.external {
			result.production = append(result.production, name)
			noteRename(&result, name, dep)
		} else if dep, ok := manifest.devDeps[name]; ok && !dep.external {
			result.production = append(result.production, name)
			noteRename(&result, name, dep)
		} else if _, ok := lookupOverride(c, name); ok {
			result.production = append(result.production, name)
		}
	}
	appendMirrorPaths(&result, manifest.normalDeps, manifest.devDeps)
	return result
}

func localCargoBuildImports(c *config.Config, manifest *cargoManifest, imports targetImports) targetImports {
	var result targetImports
	for _, name := range imports.production {
		if dep, ok := manifest.buildDeps[name]; ok && !dep.external {
			result.production = append(result.production, name)
			noteRename(&result, name, dep)
		} else if _, ok := lookupOverride(c, name); ok {
			result.production = append(result.production, name)
		}
	}
	for _, name := range imports.test {
		if dep, ok := manifest.buildDeps[name]; ok && !dep.external {
			result.test = append(result.test, name)
			noteRename(&result, name, dep)
		} else if _, ok := lookupOverride(c, name); ok {
			result.test = append(result.test, name)
		}
	}
	appendMirrorPaths(&result, manifest.buildDeps)
	return result
}

func setCargoAttrs(r *rule.Rule, packagePath string, manifest *cargoManifest, includeDev bool) {
	packageName := crateUniversePackage(packagePath, manifest)
	external := hasExternal(manifest.normalDeps) || (includeDev && hasExternal(manifest.devDeps))
	r.SetAttr("deps", crateDepsCall{call: cargoCall("all_crate_deps", packageName, includeDev), external: external})
	r.SetAttr("aliases", cargoCall("aliases", packageName, includeDev))
}

func setScriptAttrs(r *rule.Rule, packagePath string, manifest *cargoManifest) {
	packageName := crateUniversePackage(packagePath, manifest)
	if hasExternal(manifest.buildDeps) {
		call := cargoBuildCall(packageName)
		call.name = "all_crate_deps"
		r.SetAttr("deps", crateDepsCall{call: call, external: true})
	}
	r.SetAttr("aliases", cargoBuildCall(packageName))
}

func hasExternal(deps map[string]cargoDependency) bool {
	for _, dep := range deps {
		if dep.external {
			return true
		}
	}
	return false
}

func crateUniversePackage(packagePath string, manifest *cargoManifest) string {
	if manifest == nil || manifest.packageName == "" {
		return packagePath
	}
	if dir := path.Dir(packagePath); dir != "." && dir != "" {
		return dir + "/" + manifest.packageName
	}
	return manifest.packageName
}

func (l *rustLang) Resolve(c *config.Config, ix *resolve.RuleIndex, _ *repo.RemoteCache, r *rule.Rule, raw interface{}, from label.Label) {
	imports, ok := raw.(targetImports)
	if !ok {
		return
	}
	all := append([]string{}, imports.production...)
	if r.Kind() == testKind {
		all = append(all, imports.test...)
	}
	deps := make(map[string]bool)
	aliases := make(map[string]string)
	// lookup finds providers by the renamed crate name when no override exists.
	lookup := func(name string) []resolve.FindResult {
		target := name
		if crate, ok := imports.renames[name]; ok {
			target = crate
		}
		return ix.FindRulesByImportWithConfig(c, resolve.ImportSpec{Lang: languageName, Imp: target}, languageName)
	}
	noteAlias := func(name string, match resolve.FindResult) {
		if _, renamed := imports.renames[name]; !renamed {
			return
		}
		rel := match.Label.Rel(from.Repo, from.Pkg).String()
		if prev, dup := aliases[rel]; dup && prev != name {
			l.fail("rust: %s: %s is imported under conflicting aliases %q and %q", from, rel, prev, name)
			return
		}
		aliases[rel] = name
	}
	for _, name := range all {
		spec := resolve.ImportSpec{Lang: languageName, Imp: name}
		if override, found := resolve.FindRuleWithOverride(c, spec, languageName); found {
			if ignore := common.MatchingIgnore(c, languageName, name); ignore != nil {
				ignore.MarkUsed(from.Pkg)
				l.fail("rust: %s: import %q has both an exact resolve mapping and ignore", from, name)
				continue
			}
			deps[override.Rel(from.Repo, from.Pkg).String()] = true
			continue
		}
		matches := lookup(name)
		switch len(matches) {
		case 1:
			if matches[0].Label != from {
				deps[matches[0].Label.Rel(from.Repo, from.Pkg).String()] = true
				noteAlias(name, matches[0])
			}
		case 0:
			if ignore := common.MatchingIgnore(c, languageName, name); ignore != nil {
				ignore.MarkUsed(from.Pkg)
				continue
			}
			if crate, renamed := imports.renames[name]; renamed {
				l.fail("rust: %s: unresolved import %q (renamed package crate %q); add a local crate, Cargo mapping, or exact # gazelle:resolve", from, name, crate)
				continue
			}
			l.fail("rust: %s: unresolved import %q; add a local crate, Cargo mapping, or exact # gazelle:resolve", from, name)
		default:
			l.fail("rust: %s: ambiguous import %q resolves to %s", from, name, formatMatches(matches))
		}
	}
	for _, name := range imports.mirrorPaths {
		if ignore := common.MatchingIgnore(c, languageName, name); ignore != nil {
			ignore.MarkUsed(from.Pkg)
			continue
		}
		spec := resolve.ImportSpec{Lang: languageName, Imp: name}
		if override, found := resolve.FindRuleWithOverride(c, spec, languageName); found {
			deps[override.Rel(from.Repo, from.Pkg).String()] = true
			continue
		}
		if matches := lookup(name); len(matches) == 1 && matches[0].Label != from {
			deps[matches[0].Label.Rel(from.Repo, from.Pkg).String()] = true
			noteAlias(name, matches[0])
		}
	}
	for _, dep := range imports.inheritedDeps {
		if pkg, target, ok := splitDepLabel(dep, from); ok && pkg == from.Pkg {
			addLocalDep(deps, from, target)
			continue
		}
		deps[dep] = true
	}
	if imports.siblingLib != "" && imports.siblingLib != from.Name {
		addLocalDep(deps, from, imports.siblingLib)
	}
	if imports.scriptDep != "" && imports.scriptDep != from.Name {
		addLocalDep(deps, from, imports.scriptDep)
	}
	if len(aliases) > 0 {
		r.SetAttr("aliases", aliasesUnionExpr{base: r.Attr("aliases"), extra: aliases})
	}
	labels := make([]string, 0, len(deps))
	for dep := range deps {
		labels = append(labels, dep)
	}
	sort.Strings(labels)
	if len(labels) == 0 {
		return
	}
	switch existing := r.Attr("deps"); existing.(type) {
	case nil:
		r.SetAttr("deps", labels)
	case *bzl.ListExpr:
		r.SetAttr("deps", unionStrings(r.AttrStrings("deps"), labels))
	default:
		r.SetAttr("deps", depsConcatExpr{base: existing, extra: labels})
	}
}

func addLocalDep(deps map[string]bool, from label.Label, name string) {
	for dep := range deps {
		if pkg, target, ok := splitDepLabel(dep, from); ok && pkg == from.Pkg && target == name {
			delete(deps, dep)
		}
	}
	deps[":"+name] = true
}

func splitDepLabel(dep string, from label.Label) (string, string, bool) {
	rest := dep
	if strings.HasPrefix(rest, "@") {
		repo, after, found := strings.Cut(rest[1:], "//")
		if !found || (repo != "" && repo != from.Repo) {
			return "", "", false
		}
		rest = "//" + after
	}
	if strings.HasPrefix(rest, ":") {
		return from.Pkg, rest[1:], true
	}
	pkgTarget := strings.TrimPrefix(rest, "//")
	pkg, target, found := strings.Cut(pkgTarget, ":")
	if !found {
		if i := strings.LastIndex(pkg, "/"); i >= 0 {
			return pkg, pkg[i+1:], true
		}
		return pkg, pkg, true
	}
	return pkg, target, true
}

func unionStrings(a, b []string) []string { return common.UnionStrings(a, b) }

func formatMatches(matches []resolve.FindResult) string { return common.FormatMatches(matches) }
