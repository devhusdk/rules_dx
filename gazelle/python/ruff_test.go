package python

import (
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
