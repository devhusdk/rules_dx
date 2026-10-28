package common

import (
	"context"
	"slices"
	"strings"
	"testing"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
)

const testGoMod = `module example.com/app

go 1.22

require (
	github.com/google/go-cmp v0.6.0
	github.com/foo/bar/v2 v2.1.0
	github.com/foo/bar/v2/sub v0.1.0
	github.com/ind/only v1.0.0 // indirect
	example.com/local v0.0.0
	gopkg.in/yaml.v3 v3.0.1
)

replace example.com/local => ../local
`

const testMavenLock = `{
  "version": "2",
  "packages": {
    "com.google.guava:guava": ["com.google.common.base", "com.google.common.collect"],
    "junit:junit": ["org.junit", "junit.framework"],
    "org.junit.jupiter:junit-jupiter-api": ["org.junit.jupiter.api"],
    "com.split:one": ["com.split.shared"],
    "com.split:two-x": ["com.split.shared"],
    "io.netty:netty-transport-native-epoll:jar:linux-x86_64": ["io.netty.channel.epoll"]
  }
}`

func externalConfig(t *testing.T) (*config.Config, string) {
	t.Helper()
	root := t.TempDir()
	cfg := config.New()
	cfg.RepoRoot = root
	return cfg, root
}

func directiveFile(t *testing.T, pkg, content string) *rule.File {
	t.Helper()
	f, err := rule.LoadData("BUILD.bazel", pkg, []byte(content))
	if err != nil {
		t.Fatal(err)
	}
	return f
}

func labelStrings(labels []label.Label) string {
	var out []string
	for _, l := range labels {
		out = append(out, l.String())
	}
	return strings.Join(out, ",")
}

func TestConfigureExternalDirectiveErrors(t *testing.T) {
	cfg, root := externalConfig(t)
	dotFile(t, root, "bad/go.mod", "require (\n")
	errs := ConfigureExternal(cfg, "app", directiveFile(t, "app", strings.Join([]string{
		"# gazelle:dx_external go",
		"# gazelle:dx_external cobol x.lock",
		"# gazelle:dx_external go go.mod repo",
		"# gazelle:dx_external maven missing.json",
		"# gazelle:dx_external go bad/go.mod",
		"# gazelle:dx_external maven off maven",
		"",
	}, "\n")))
	want := []string{"malformed", `unsupported ecosystem "cobol"`, "takes no repo", "missing.json: no such file", "bad/go.mod", "off takes no repo"}
	if len(errs) != len(want) {
		t.Fatalf("errors = %v", errs)
	}
	for i, w := range want {
		if !strings.HasPrefix(errs[i], "dx_external: //app: ") || !strings.Contains(errs[i], w) {
			t.Errorf("error %d = %q, want %q", i, errs[i], w)
		}
	}
	if errs := ConfigureExternal(cfg, "app", directiveFile(t, "app", "# gazelle:dx_external go\n")); errs != nil {
		t.Errorf("second configure of the same rel = %v", errs)
	}
}

func TestConfigureExternalDiscoveryAndInheritance(t *testing.T) {
	cfg, root := externalConfig(t)
	dotFile(t, root, "mod/go.mod", testGoMod)
	dotFile(t, root, "mod/bad/go.mod", "require (\n")
	dotFile(t, root, "third_party/a.json", testMavenLock)
	dotFile(t, root, "third_party/b.json", `{"packages": {"org.other:lib": ["org.other"]}}`)

	if errs := ConfigureExternal(cfg, "", nil); errs != nil {
		t.Fatal(errs)
	}
	if got, _ := ExternalLabels(cfg, "go", "github.com/google/go-cmp/cmp"); got != nil {
		t.Errorf("root without go.mod resolved %v", got)
	}

	mod := cfg.Clone()
	if errs := ConfigureExternal(mod, "mod", nil); errs != nil {
		t.Fatal(errs)
	}
	if got, _ := ExternalLabels(mod, "go", "github.com/google/go-cmp/cmp"); labelStrings(got) != "@com_github_google_go_cmp//cmp" {
		t.Errorf("discovered go.mod gives %v", got)
	}
	if got, _ := ExternalLabels(cfg, "go", "github.com/google/go-cmp/cmp"); got != nil {
		t.Errorf("child mutated the parent: %v", got)
	}

	child := mod.Clone()
	if errs := ConfigureExternal(child, "mod/sub", nil); errs != nil {
		t.Fatal(errs)
	}
	if got := ExternalSources(child, "go"); !slices.Equal(got, []string{"mod/go.mod"}) {
		t.Errorf("inherited sources = %v", got)
	}

	bad := mod.Clone()
	if errs := ConfigureExternal(bad, "mod/bad", nil); errs != nil {
		t.Errorf("an unparseable discovered file reported %v", errs)
	}

	off := mod.Clone()
	if errs := ConfigureExternal(off, "mod/off", directiveFile(t, "mod/off", "# gazelle:dx_external go off\n")); errs != nil {
		t.Fatal(errs)
	}
	if got := ExternalSources(off, "go"); got != nil {
		t.Errorf("off kept %v", got)
	}

	multi := cfg.Clone()
	errs := ConfigureExternal(multi, "jvm", directiveFile(t, "jvm", "# gazelle:dx_external maven third_party/a.json\n# gazelle:dx_external maven //third_party/b.json hub\n"))
	if errs != nil {
		t.Fatal(errs)
	}
	if got := ExternalSources(multi, "maven"); !slices.Equal(got, []string{"third_party/a.json", "third_party/b.json"}) {
		t.Errorf("repeated lines = %v", got)
	}
	if got, _ := ExternalLabels(multi, "maven", "org.other.Thing"); labelStrings(got) != "@hub//:org_other_lib" {
		t.Errorf("second source = %v", got)
	}

	replaced := multi.Clone()
	if errs := ConfigureExternal(replaced, "jvm/sub", directiveFile(t, "jvm/sub", "# gazelle:dx_external maven third_party/b.json\n")); errs != nil {
		t.Fatal(errs)
	}
	if got := ExternalSources(replaced, "maven"); !slices.Equal(got, []string{"third_party/b.json"}) {
		t.Errorf("replaced sources = %v", got)
	}
	if got := ExternalSources(multi, "maven"); len(got) != 2 {
		t.Errorf("replacement mutated the parent: %v", got)
	}
}

func TestExternalGoMod(t *testing.T) {
	cfg, root := externalConfig(t)
	dotFile(t, root, "go.mod", testGoMod)
	if errs := ConfigureExternal(cfg, "", nil); errs != nil {
		t.Fatal(errs)
	}
	for _, tc := range []struct{ imp, want, hint string }{
		{"github.com/google/go-cmp/cmp", "@com_github_google_go_cmp//cmp", ""},
		{"github.com/google/go-cmp/cmp/cmpopts", "@com_github_google_go_cmp//cmp/cmpopts", ""},
		{"github.com/foo/bar/v2", "@com_github_foo_bar_v2//:bar", ""},
		{"github.com/foo/bar/v2/x", "@com_github_foo_bar_v2//x", ""},
		{"github.com/foo/bar/v2/sub/y", "@com_github_foo_bar_v2_sub//y", ""},
		{"gopkg.in/yaml.v3", "@in_gopkg_yaml_v3//:yaml_v3", ""},
		{"github.com/ind/only/pkg", "", "only an indirect require"},
		{"example.com/local/pkg", "", ""},
		{"example.com/app/inner", "", ""},
		{"github.com/google/go-cmpx", "", ""},
	} {
		got, hint := ExternalLabels(cfg, "go", tc.imp)
		if labelStrings(got) != tc.want || !strings.Contains(hint, tc.hint) || (tc.hint == "" && hint != "") {
			t.Errorf("%s: labels = %v hint = %q, want %q %q", tc.imp, got, hint, tc.want, tc.hint)
		}
	}
}

func TestExternalMaven(t *testing.T) {
	for _, tc := range []struct{ coord, want string }{
		{"com.google.guava:guava", "com_google_guava_guava"},
		{"org.scala-lang:scala-library", "org_scala_lang_scala_library"},
		{"io.netty:netty-transport-native-epoll:jar:linux-x86_64", "io_netty_netty_transport_native_epoll_linux_x86_64"},
		{"a.b:c:jar:sources", "a_b_c"},
		{"a.b:c:zip", "a_b_c"},
		{"a.b:c:zip:cls", "a_b_c_zip_cls"},
	} {
		if got := MavenTargetName(tc.coord); got != tc.want {
			t.Errorf("%s: target = %q, want %q", tc.coord, got, tc.want)
		}
	}
	cfg, root := externalConfig(t)
	dotFile(t, root, "maven_install.json", testMavenLock)
	if errs := ConfigureExternal(cfg, "", nil); errs != nil {
		t.Fatal(errs)
	}
	for _, tc := range []struct{ imp, want string }{
		{"com.google.common.collect.ImmutableList", "@maven//:com_google_guava_guava"},
		{"com.google.common.collect.ImmutableList.Builder", "@maven//:com_google_guava_guava"},
		{"com.google.common.base.*", "@maven//:com_google_guava_guava"},
		{"com.google.common.collect", "@maven//:com_google_guava_guava"},
		{"org.junit.jupiter.api.Test", "@maven//:org_junit_jupiter_junit_jupiter_api"},
		{"org.junit.jupiter.engine.Thing", ""},
		{"org.junit.Assert.assertEquals", "@maven//:junit_junit"},
		{"io.netty.channel.epoll.Epoll", "@maven//:io_netty_netty_transport_native_epoll_linux_x86_64"},
		{"com.split.shared.Thing", "@maven//:com_split_one,@maven//:com_split_two_x"},
		{"com.google.common.Other", ""},
	} {
		got, _ := ExternalLabels(cfg, "maven", tc.imp)
		if labelStrings(got) != tc.want {
			t.Errorf("%s: labels = %v, want %q", tc.imp, got, tc.want)
		}
	}
	if errs := ConfigureExternal(config.New(), "x", directiveFile(t, "x", "# gazelle:dx_external maven "+root+"/../nope.json\n")); len(errs) != 1 {
		t.Errorf("missing file errors = %v", errs)
	}
	dotFile(t, root, "old.json", `{"artifacts": {}}`)
	sub := cfg.Clone()
	if errs := ConfigureExternal(sub, "sub", directiveFile(t, "sub", "# gazelle:dx_external maven old.json\n")); len(errs) != 1 || !strings.Contains(errs[0], "no packages map") {
		t.Errorf("lock without packages = %v", errs)
	}
}

func newExternalDotLang() *SingleLang {
	spec := dotSpec()
	spec.Candidates = JVMCandidates
	spec.External = "maven"
	return NewSingleLang(spec)
}

func TestResolveSingleExternal(t *testing.T) {
	root := t.TempDir()
	dotFile(t, root, "maven_install.json", testMavenLock)
	l := newExternalDotLang()
	if got := strings.Join(l.KnownDirectives(), ","); got != "dx_ignore_import,dx_external" {
		t.Errorf("directives = %q", got)
	}
	l.Before(context.Background())
	cfg := resolveConfig(t, root, "# gazelle:resolve dot dot com.google.common.collect.Mapped //mapped:dep\n")
	l.Configure(cfg, "", nil)
	l.Configure(cfg, "app", directiveFile(t, "app", "# gazelle:dx_ignore_import dot com.google.common.collect.Ignored\n"))
	index := resolverIndex(l, struct{ pkg, name string }{"lib/local", "com.google.common.base"})
	from := label.New("", "app", "app")
	for _, tc := range []struct{ imp, want string }{
		{"com.google.common.collect.Mapped", "//mapped:dep"},
		{"com.google.common.collect.Ignored", ""},
		{"com.google.common.base.Strings", "//lib/local:com.google.common.base"},
		{"com.google.common.collect.ImmutableList", "@maven//:com_google_guava_guava"},
	} {
		r := rule.NewRule(dotKind, "app")
		l.Resolve(cfg, index, nil, r, WrapImportSet([]string{tc.imp}), from)
		if got := strings.Join(r.AttrStrings("deps"), ","); got != tc.want {
			t.Errorf("%s: deps = %q, want %q", tc.imp, got, tc.want)
		}
	}
	if len(l.errors) != 0 {
		t.Fatalf("errors = %v", l.errors)
	}
	if entry := MatchingIgnore(cfg, dotLanguage, "com.google.common.collect.Ignored"); entry == nil || !entry.Used() {
		t.Errorf("ignore was not marked used: %+v", entry)
	}
	l.Resolve(cfg, index, nil, rule.NewRule(dotKind, "app"), WrapImportSet([]string{"com.split.shared.X", "org.nothing.Y"}), from)
	if len(l.errors) != 2 || !strings.Contains(l.errors[0].Message, "ambiguous import") || !strings.Contains(l.errors[0].Message, "@maven//:com_split_one, @maven//:com_split_two_x") || !strings.Contains(l.errors[1].Message, `unresolved import "org.nothing.Y"`) {
		t.Errorf("errors = %v", l.errors)
	}
}

func TestExternalPythonUVLock(t *testing.T) {
	cfg, root := externalConfig(t)
	dotFile(t, root, "uv.lock", `version = 1

[[package]]
name = "My_Pkg"
version = "1.0"
source = { registry = "https://pypi.org/simple" }

[[package]]
name = "app"
version = "0.1.0"
source = { virtual = "." }
`)
	if errs := ConfigureExternal(cfg, "", nil); errs != nil {
		t.Fatal(errs)
	}
	for imp, want := range map[string]string{"my_pkg": "@pypi//my-pkg", "my_pkg.sub": "@pypi//my-pkg", "app": "", "other": ""} {
		got, _ := ExternalLabels(cfg, "python", imp)
		if labelStrings(got) != want {
			t.Errorf("%s: got %v, want %q", imp, got, want)
		}
	}
}

func TestExternalNpmRootOnly(t *testing.T) {
	cfg, root := externalConfig(t)
	dotFile(t, root, "package.json", `{"dependencies":{"react":"1"},"devDependencies":{"@scope/pkg":"1"}}`)
	dotFile(t, root, "sub/package.json", `{"dependencies":{"vue":"1"}}`)
	if errs := ConfigureExternal(cfg, "", nil); errs != nil {
		t.Fatal(errs)
	}
	sub := cfg.Clone()
	if errs := ConfigureExternal(sub, "sub", nil); errs != nil {
		t.Fatal(errs)
	}
	for imp, want := range map[string]string{"react": "//:node_modules/react", "react/jsx-runtime": "//:node_modules/react", "@scope/pkg/x": "//:node_modules/@scope/pkg", "vue": ""} {
		got, _ := ExternalLabels(sub, "npm", imp)
		if labelStrings(got) != want {
			t.Errorf("%s: got %v, want %q", imp, got, want)
		}
	}
}
