package python

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/devhusdk/rules_dx/gazelle/common"
)

func policyConfig(t *testing.T, directives string) *config.Config {
	t.Helper()
	file, err := rule.LoadData("BUILD.bazel", "", []byte(directives))
	if err != nil {
		t.Fatal(err)
	}
	cfg := config.New()
	common.ConfigurePolicyConfig(cfg, file)
	return cfg
}

func TestRuffHintSkippedWhenPolicyBindsConfig(t *testing.T) {
	cfg := policyConfig(t, "# gazelle:dx_policy_config ruff //:ruff_config\n")
	if got := withRuffHint(cfg, nil, &ruffState{rel: ""}, "pkg"); len(got) != 0 {
		t.Errorf("bound root config hinted: %v", got)
	}
}

func TestRuffHintKeptForNestedOverride(t *testing.T) {
	cfg := policyConfig(t, "# gazelle:dx_policy_config ruff //:ruff_config\n")
	got := withRuffHint(cfg, nil, &ruffState{rel: "pkg"}, "pkg")
	if strings.Join(got, ",") != ":ruff_config" {
		t.Errorf("override hints = %v, want [:ruff_config]", got)
	}
}

func TestRuffHintKeepsHandWrittenHint(t *testing.T) {
	cfg := policyConfig(t, "# gazelle:dx_policy_config ruff //:ruff_config\n")
	existing := rule.NewRule(libraryKind, "lib")
	existing.SetAttr("aspect_hints", []string{"//:ruff_config"})
	got := withRuffHint(cfg, existing, &ruffState{rel: ""}, "pkg")
	if strings.Join(got, ",") != "//:ruff_config" {
		t.Errorf("hints = %v, want hand-written hint kept", got)
	}
}

func TestRuffHintDroppedWhenInitOwnsConfig(t *testing.T) {
	existing := rule.NewRule(libraryKind, "lib")
	existing.SetAttr("aspect_hints", []string{"//:user_hint", "//:ruff_config"})
	got := withRuffHint(config.New(), existing, &ruffState{rel: "", initOwned: true}, "pkg")
	if strings.Join(got, ",") != "//:user_hint" {
		t.Errorf("hints = %v, want only the user hint", got)
	}
	existing.SetAttr("aspect_hints", []string{":ruff_config"})
	if got := withRuffHint(config.New(), existing, &ruffState{rel: "", initOwned: true}, ""); len(got) != 0 {
		t.Errorf("root hints = %v, want none", got)
	}
}

func TestRuffHintKeptForNestedOverrideWhenInitOwned(t *testing.T) {
	root := t.TempDir()
	writeFile(t, filepath.Join(root, "ruff.toml"), "line-length = 100\n")
	writeFile(t, filepath.Join(root, "pkg", "ruff.toml"), "line-length = 80\n")
	writeFile(t, filepath.Join(root, ".dx", "config", "BUILD.bazel"),
		"ruff_config(\n    name = \"ruff\",\n    src = \"//:ruff.toml\",\n)\n")
	cfg := config.New()
	cfg.RepoRoot = root
	configureRuff(cfg, "", nil, nil)
	nested := cfg.Clone()
	configureRuff(nested, "pkg", nil, nil)
	if ruffAt(nested).initOwned {
		t.Fatal("nested override marked init-owned")
	}
	got := withRuffHint(nested, nil, ruffAt(nested), "pkg")
	if strings.Join(got, ",") != ":ruff_config" {
		t.Errorf("override hints = %v, want [:ruff_config]", got)
	}
}

func TestConfigureRuffInitOwned(t *testing.T) {
	for _, tc := range []struct {
		src  string
		want bool
	}{
		{"//:ruff.toml", true},
		{"//:pyproject.toml", false},
	} {
		root := t.TempDir()
		writeFile(t, filepath.Join(root, "ruff.toml"), "line-length = 100\n")
		writeFile(t, filepath.Join(root, ".dx", "config", "BUILD.bazel"),
			"ruff_config(\n    name = \"ruff\",\n    src = \""+tc.src+"\",\n)\n")
		cfg := config.New()
		cfg.RepoRoot = root
		configureRuff(cfg, "", nil, nil)
		if got := ruffAt(cfg).initOwned; got != tc.want {
			t.Errorf("src %s: initOwned = %v, want %v", tc.src, got, tc.want)
		}
		if got := withRuffHint(cfg, nil, ruffAt(cfg), "pkg"); (len(got) == 0) != tc.want {
			t.Errorf("src %s: hints = %v", tc.src, got)
		}
	}
}

func writeFile(t *testing.T, path, content string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}
