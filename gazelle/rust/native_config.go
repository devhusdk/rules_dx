package rust

import (
	"fmt"
	"os"
	"path"
	"path/filepath"
	"sort"
	"strings"

	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/rule"
)

const nativeToolsDirective = "dx_native_tools"

type nativeTool struct {
	id   string
	file string
	kind string
	rust bool
}

var nativeTools = []nativeTool{
	{id: "buildifier", file: ".buildifier.json", kind: "buildifier_config"},
	{id: "taplo", file: "taplo.toml", kind: "taplo_config"},
	{id: "vale", file: ".vale.ini", kind: "vale_config"},
	{id: "rustfmt", file: "rustfmt.toml", kind: "rustfmt_config", rust: true},
}

func nativeToolByID(id string) (nativeTool, bool) {
	for _, tool := range nativeTools {
		if tool.id == id {
			return tool, true
		}
	}
	return nativeTool{}, false
}

func nativeToolByKind(kind string) (nativeTool, bool) {
	for _, tool := range nativeTools {
		if tool.kind == kind {
			return tool, true
		}
	}
	return nativeTool{}, false
}

func nativeConfigName(tool nativeTool) string {
	return tool.id + "_config"
}

func defaultNativeTools() []string {
	ids := make([]string, 0, len(nativeTools))
	for _, tool := range nativeTools {
		ids = append(ids, tool.id)
	}
	return ids
}

func nativeToolIDs() string {
	return strings.Join(defaultNativeTools(), " ")
}

func parseNativeToolsDirective(value string) ([]string, error) {
	fields := strings.Fields(value)
	if len(fields) == 0 {
		return nil, fmt.Errorf("rust: malformed # gazelle:%s: want space-separated tool ids (%s)", nativeToolsDirective, nativeToolIDs())
	}
	seen := make(map[string]bool, len(fields))
	tools := make([]string, 0, len(fields))
	for _, field := range fields {
		tool, ok := nativeToolByID(field)
		if !ok {
			return nil, fmt.Errorf("rust: unknown native tool %q in # gazelle:%s (want one of: %s)", field, nativeToolsDirective, nativeToolIDs())
		}
		if !seen[tool.id] {
			seen[tool.id] = true
			tools = append(tools, tool.id)
		}
	}
	return tools, nil
}

func selectedNativeTools(c *config.Config) []string {
	if c != nil {
		if raw, ok := c.Exts[languageName]; ok {
			if rc, ok := raw.(*rustConfig); ok && rc != nil && rc.tools != nil {
				return rc.tools
			}
		}
	}
	return defaultNativeTools()
}

func nativeConfigKinds() map[string]rule.KindInfo {
	kinds := make(map[string]rule.KindInfo, len(nativeTools))
	for _, tool := range nativeTools {
		kinds[tool.kind] = rule.KindInfo{
			MatchAttrs:    []string{"src"},
			NonEmptyAttrs: map[string]bool{"src": true},
			MergeableAttrs: map[string]bool{
				"data": true,
				"src":  true,
			},
		}
	}
	return kinds
}

func isNativeConfigKind(kind string) bool {
	_, ok := nativeToolByKind(kind)
	return ok
}

func nativeConfigLoads(rulesRepo string) rule.LoadInfo {
	symbols := make([]string, 0, len(nativeTools))
	for _, tool := range nativeTools {
		symbols = append(symbols, tool.kind)
	}
	return rule.LoadInfo{
		Name:    "@" + rulesRepo + "//quality:native_config.bzl",
		Symbols: symbols,
	}
}

type nativePlan struct {
	gen       []*rule.Rule
	imports   []any
	empty     []*rule.Rule
	resolved  map[string]string
	generated map[string]bool
	stubbed   map[string]bool
	existing  map[string]*rule.Rule
}

func planNativeConfig(c *config.Config, args language.GenerateArgs) (*nativePlan, error) {
	plan := &nativePlan{
		resolved:  make(map[string]string, len(nativeTools)),
		generated: make(map[string]bool),
		stubbed:   make(map[string]bool),
		existing:  make(map[string]*rule.Rule),
	}
	if args.File != nil {
		for _, existing := range args.File.Rules {
			plan.existing[existing.Name()] = existing
		}
	}
	present := make(map[string]bool, len(args.RegularFiles))
	for _, name := range args.RegularFiles {
		present[name] = true
	}
	selected := make(map[string]bool, len(nativeTools))
	for _, id := range selectedNativeTools(c) {
		selected[id] = true
	}
	for _, tool := range nativeTools {
		var owned []string
		for name, existing := range plan.existing {
			if existing.Kind() == tool.kind {
				owned = append(owned, name)
			}
		}
		if len(owned) > 1 {
			sort.Strings(owned)
			return nil, fmt.Errorf("rust: //%s: tool %q has %d config targets (%s); keep one so aspect_hints stays unambiguous",
				args.Rel, tool.id, len(owned), strings.Join(owned, ", "))
		}
		name := nativeConfigName(tool)
		if len(owned) == 1 {
			plan.resolved[tool.id] = owned[0]
			continue
		}
		if !selected[tool.id] || !present[tool.file] {
			continue
		}
		r := rule.NewRule(tool.kind, name)
		r.SetAttr("src", tool.file)
		if data := valeStylesData(args.Config.RepoRoot, args.Dir, args.Rel, tool); len(data) > 0 {
			r.SetAttr("data", data)
		}
		r.SetAttr("visibility", []string{nativeVisibility(args.Rel)})
		plan.gen = append(plan.gen, r)
		plan.imports = append(plan.imports, targetImports{})
		plan.generated[name] = true
		plan.resolved[tool.id] = name
	}
	for _, tool := range nativeTools {
		name := nativeConfigName(tool)
		existing, ok := plan.existing[name]
		if !ok || existing.Kind() != tool.kind || plan.generated[name] {
			continue
		}
		if existing.AttrString("src") != tool.file {
			continue
		}
		if selected[tool.id] && present[tool.file] {
			continue
		}
		plan.empty = append(plan.empty, rule.NewRule(tool.kind, name))
		plan.stubbed[name] = true
		if plan.resolved[tool.id] == name {
			delete(plan.resolved, tool.id)
		}
	}
	return plan, nil
}

func nativeVisibility(rel string) string {
	if rel == "" {
		return "//:__subpackages__"
	}
	return "//" + rel + ":__subpackages__"
}

func applyNativeHints(rel string, r *rule.Rule, plan *nativePlan) {
	if _, owned := rustKinds[r.Kind()]; !owned || isNativeConfigKind(r.Kind()) || r.Kind() == corpusKind {
		return
	}
	existing := plan.existing[r.Name()]
	if existing != nil && existing.Kind() != r.Kind() {
		existing = nil
	}
	var merged []string
	if existing != nil {
		for _, entry := range existing.AttrStrings("aspect_hints") {
			name, managed := managedHintName(rel, entry)
			if managed && !plan.targetPresent(name) {
				continue
			}
			merged = append(merged, entry)
		}
	}
	present := make(map[string]bool, len(merged))
	for _, entry := range merged {
		present[hintCanonicalName(rel, entry)] = true
	}
	var additions []string
	for _, tool := range nativeTools {
		if !tool.rust {
			continue
		}
		name, ok := plan.resolved[tool.id]
		if !ok || present[name] {
			continue
		}
		additions = append(additions, ":"+name)
	}
	sort.Strings(additions)
	merged = append(merged, additions...)
	if len(merged) == 0 {
		return
	}
	r.SetAttr("aspect_hints", merged)
}

func (plan *nativePlan) targetPresent(name string) bool {
	if plan.generated[name] {
		return true
	}
	if _, ok := plan.existing[name]; !ok || plan.stubbed[name] {
		return false
	}
	return true
}

func managedHintName(rel, entry string) (string, bool) {
	for _, tool := range nativeTools {
		if !tool.rust {
			continue
		}
		name := nativeConfigName(tool)
		if entry == ":"+name {
			return name, true
		}
		if rel == "" {
			if entry == "//:"+name {
				return name, true
			}
		} else if entry == "//"+rel+":"+name {
			return name, true
		}
	}
	return "", false
}

func hintCanonicalName(rel, entry string) string {
	if name, managed := managedHintName(rel, entry); managed {
		return name
	}
	return entry
}

func valeStylesData(repoRoot, dir, rel string, tool nativeTool) []string {
	if tool.id != "vale" {
		return nil
	}
	content, err := os.ReadFile(filepath.Join(dir, tool.file))
	if err != nil {
		return nil
	}
	styles := "styles"
	for _, line := range strings.Split(string(content), "\n") {
		trimmed := strings.TrimSpace(line)
		if trimmed == "" || strings.HasPrefix(trimmed, "#") || strings.HasPrefix(trimmed, ";") || strings.HasPrefix(trimmed, "[") {
			continue
		}
		key, value, ok := strings.Cut(trimmed, "=")
		if !ok || !strings.EqualFold(strings.TrimSpace(key), "StylesPath") {
			continue
		}
		styles = strings.Trim(strings.TrimSpace(value), `"'`)
	}
	if styles == "" || path.IsAbs(styles) || filepath.IsAbs(styles) {
		return nil
	}
	var data []string
	root := filepath.Join(dir, filepath.FromSlash(styles))
	_ = filepath.WalkDir(root, func(name string, entry os.DirEntry, err error) error {
		if err != nil || entry.IsDir() {
			return nil
		}
		local, relErr := filepath.Rel(dir, name)
		if relErr != nil {
			return nil // LCOV_EXCL_LINE - reason: defensive branch, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
		}
		slash := filepath.ToSlash(local)
		if slash == ".." || strings.HasPrefix(slash, "../") {
			return nil
		}
		for d := path.Dir(slash); d != "."; d = path.Dir(d) {
			for _, build := range []string{"BUILD.bazel", "BUILD"} {
				if _, statErr := os.Stat(filepath.Join(dir, filepath.FromSlash(d), build)); statErr == nil {
					return nil
				}
			}
		}
		data = append(data, slash)
		return nil
	})
	sort.Strings(data)
	return data
}
