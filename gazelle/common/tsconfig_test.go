package common

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/repo"
	"github.com/bazelbuild/bazel-gazelle/resolve"
)

func writeFiles(t *testing.T, root string, files map[string]string) {
	t.Helper()
	for name, content := range files {
		p := filepath.Join(root, filepath.FromSlash(name))
		if err := os.MkdirAll(filepath.Dir(p), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(p, []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
	}
}

func candidatesFor(t *testing.T, root, rel, spec string) string {
	t.Helper()
	paths, found, err := LoadTSPaths(root, rel)
	if err != nil || !found {
		t.Fatalf("LoadTSPaths(%q) = found %v, err %v", rel, found, err)
	}
	return strings.Join(paths.Candidates(spec), ",")
}

func TestLoadTSPaths(t *testing.T) {
	root := t.TempDir()
	writeFiles(t, root, map[string]string{
		"str/tsconfig.json":  `{"extends": "./base", "compilerOptions": {}}`,
		"str/base.json":      `{"compilerOptions": {"paths": {"@/*": ["./src/*"]}}}`,
		"arr/tsconfig.json":  `{"extends": ["./a.json", "./b.json"]}`,
		"arr/a.json":         `{"compilerOptions": {"paths": {"@/*": ["./a/*"]}, "baseUrl": "."}}`,
		"arr/b.json":         `{"compilerOptions": {"paths": {"@/*": ["./b/*"]}}}`,
		"pkg/tsconfig.json":  `{"extends": "@tsconfig/missing", "compilerOptions": {"paths": {"x": ["./x.ts"]}}}`,
		"node/tsconfig.json": `{"extends": "@tsconfig/base", "compilerOptions": {}}`,
		"node_modules/@tsconfig/base/tsconfig.json": `{"compilerOptions": {"paths": {"n": ["${configDir}/n.ts"]}}}`,
		"missing/tsconfig.json":                     `{"extends": "./nope"}`,
		"cycle/tsconfig.json":                       `{"extends": "./other.json"}`,
		"cycle/other.json":                          `{"extends": "./tsconfig.json"}`,
		"jsonc/tsconfig.json":                       "{\n  // comment\n  \"compilerOptions\": { /* block */ \"paths\": {\"a//b\": [\"./c\",],},\n},\n}\n",
		"js/jsconfig.json":                          `{"compilerOptions": {"paths": {"~/*": ["./*"]}}}`,
		"cd/tsconfig.json":                          `{"extends": "../shared/base.json"}`,
		"shared/base.json":                          `{"compilerOptions": {"paths": {"@/*": ["${configDir}/src/*"]}}}`,
		"esc/tsconfig.json":                         `{"compilerOptions": {"paths": {"out/*": ["../../*", "/abs/*"], "in/*": ["./*"]}}}`,
		"mix/tsconfig.json":                         `{"extends": "./base.json", "compilerOptions": {"paths": {"@/*": ["./*"]}}}`,
		"mix/base.json":                             `{"compilerOptions": {"baseUrl": "./root"}}`,
		"broken/tsconfig.json":                      `{"compilerOptions": `,
	})
	for _, tc := range []struct{ rel, spec, want string }{
		{"str", "@/x", "str/src/x"},
		{"arr", "@/x", "arr/b/x,arr/@/x"},
		{"pkg", "x", "pkg/x.ts"},
		{"node", "n", "node/n.ts"},
		{"jsonc", "a//b", "jsonc/c"},
		{"js", "~/lib", "js/lib"},
		{"cd", "@/y", "cd/src/y"},
		{"esc", "out/z", ""},
		{"esc", "in/../../z", ""},
		{"mix", "@/m", "mix/root/m,mix/root/@/m"},
	} {
		if got := candidatesFor(t, root, tc.rel, tc.spec); got != tc.want {
			t.Errorf("%s %q = %q, want %q", tc.rel, tc.spec, got, tc.want)
		}
	}
	for _, rel := range []string{"missing", "cycle", "broken"} {
		if _, found, err := LoadTSPaths(root, rel); err == nil || !found {
			t.Errorf("%s: found %v, err %v; want an error", rel, found, err)
		} else if !strings.HasPrefix(err.Error(), "tsconfig: //"+rel+"/tsconfig.json: ") {
			t.Errorf("%s: error %q", rel, err)
		}
	}
	if _, found, err := LoadTSPaths(root, "none"); found || err != nil {
		t.Errorf("none: found %v, err %v", found, err)
	}
}

func TestTSPathsCandidates(t *testing.T) {
	root := t.TempDir()
	writeFiles(t, root, map[string]string{
		"tsconfig.json": `{"compilerOptions": {"paths": {
			"@/*": ["./src/*", "./gen/*"],
			"@/deep/*": ["./deep/*"],
			"~utils": ["./lib/utils.ts"],
			"*.css": ["./styles/*.css"]
		}}}`,
		"base/tsconfig.json": `{"compilerOptions": {"baseUrl": "./src"}}`,
	})
	for _, tc := range []struct{ rel, spec, want string }{
		{"", "@/a", "src/a,gen/a"},
		{"", "@/deep/b", "deep/b"},
		{"", "~utils", "lib/utils.ts"},
		{"", "~utils/x", ""},
		{"", "app.css", "styles/app.css"},
		{"", "react", ""},
		{"base", "lib/x", "base/src/lib/x"},
	} {
		if got := candidatesFor(t, root, tc.rel, tc.spec); got != tc.want {
			t.Errorf("%s %q = %q, want %q", tc.rel, tc.spec, got, tc.want)
		}
	}
	var none *TSPaths
	if got := none.Candidates("x"); got != nil {
		t.Errorf("nil candidates = %v", got)
	}
}

type jsFakeResolver struct{}

func (jsFakeResolver) Name() string { return "js" }
func (jsFakeResolver) Imports(_ *config.Config, r *rule.Rule, _ *rule.File) []resolve.ImportSpec {
	var out []resolve.ImportSpec
	for _, imp := range r.AttrStrings("imps") {
		out = append(out, resolve.ImportSpec{Lang: "js", Imp: imp})
	}
	return out
}
func (jsFakeResolver) Embeds(*rule.Rule, label.Label) []label.Label { return nil }
func (jsFakeResolver) Resolve(*config.Config, *resolve.RuleIndex, *repo.RemoteCache, *rule.Rule, interface{}, label.Label) {
}

func TestFindJSImportTSPaths(t *testing.T) {
	root := t.TempDir()
	writeFiles(t, root, map[string]string{
		"tsconfig.json": `{"compilerOptions": {"paths": {"@/*": ["./*"], "~types": ["./types/api.d.ts"], "util": ["./lib/util.ts"]}}}`,
	})
	cfg := resolveConfig(t, root, "# gazelle:resolve js @/mapped //third_party:mapped\n# gazelle:resolve js other/x //third_party:cand\n")
	if err := ConfigureTSPaths(cfg, ""); err != nil {
		t.Fatal(err)
	}
	index := resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return jsFakeResolver{} })
	for _, entry := range [][3]string{
		{"lib", "util", "lib/util"},
		{"lib/components", "index", "lib/components/index"},
		{"types", "api", "types/api"},
		{"mapped", "mapped", "mapped"},
		{"pkg", "pkg", "pkg"},
	} {
		r := rule.NewRule("js_library", entry[1])
		r.SetAttr("imps", []string{entry[2]})
		index.AddRule(config.New(), r, rule.EmptyFile(filepath.Join(entry[0], "BUILD.bazel"), entry[0]))
	}
	index.Finish()
	for _, tc := range []struct {
		name  string
		local bool
		want  string
	}{
		{"@/lib/util", false, "//lib:util lib/util"},
		{"@/lib/util.ts", false, "//lib:util lib/util"},
		{"@/lib/components", false, "//lib/components:index lib/components/index"},
		{"~types", false, "//types:api types/api"},
		{"@/mapped", false, "override //third_party:mapped @/mapped"},
		{"@/other/x", false, "override //third_party:cand other/x"},
		{"pkg", false, "//pkg pkg"},
		{"@/missing", false, "none @/missing"},
		{"lib/util", true, "//lib:util lib/util"},
		{"@/lib/util", true, "none @/lib/util"},
	} {
		found := FindJSImport(cfg, index, "js", tc.name, tc.local)
		var got string
		switch {
		case found.HasOverride:
			got = "override " + found.Override.String() + " " + found.Matched
		case len(found.Matches) == 1:
			got = found.Matches[0].Label.String() + " " + found.Matched
		default:
			got = "none " + found.Matched
		}
		if got != tc.want {
			t.Errorf("%q local=%v = %q, want %q", tc.name, tc.local, got, tc.want)
		}
	}
}

func TestConfigureTSPathsOnce(t *testing.T) {
	root := t.TempDir()
	writeFiles(t, root, map[string]string{
		"tsconfig.json":       `{"compilerOptions": {"paths": {"@/*": ["./*"]}}}`,
		"bad/tsconfig.json":   `{`,
		"clear/tsconfig.json": `{}`,
	})
	parent := config.New()
	parent.RepoRoot = root
	if err := ConfigureTSPaths(parent, ""); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "tsconfig.json"), []byte("{"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := ConfigureTSPaths(parent, ""); err != nil {
		t.Errorf("second call reloaded: %v", err)
	}
	child := parent.Clone()
	if err := ConfigureTSPaths(child, "app"); err != nil {
		t.Fatal(err)
	}
	if got := strings.Join(tsPathsFor(child).Candidates("@/x"), ","); got != "x" {
		t.Errorf("inherited candidates = %q", got)
	}
	cleared := parent.Clone()
	if err := ConfigureTSPaths(cleared, "clear"); err != nil {
		t.Fatal(err)
	}
	if got := tsPathsFor(cleared).Candidates("@/x"); len(got) != 0 {
		t.Errorf("cleared candidates = %v", got)
	}
	if got := strings.Join(tsPathsFor(parent).Candidates("@/x"), ","); got != "x" {
		t.Errorf("parent leaked child state: %q", got)
	}
	bad := parent.Clone()
	if err := ConfigureTSPaths(bad, "bad"); err == nil || !strings.HasPrefix(err.Error(), "tsconfig: //bad/tsconfig.json: ") {
		t.Errorf("bad error = %v", err)
	}
	if err := ConfigureTSPaths(bad, "bad"); err != nil {
		t.Errorf("bad reported twice: %v", err)
	}
}

func TestUnmarshalJSONC(t *testing.T) {
	in := "{\"a\": \"// not a comment\", /* c */ \"b\": [1, 2,], // tail\n \"c\": \"x,]\",}"
	var got struct {
		A string `json:"a"`
		B []int  `json:"b"`
		C string `json:"c"`
	}
	if err := unmarshalJSONC([]byte(in), &got); err != nil {
		t.Fatal(err)
	}
	if got.A != "// not a comment" || len(got.B) != 2 || got.C != "x,]" {
		t.Errorf("unmarshalJSONC = %+v", got)
	}
}
