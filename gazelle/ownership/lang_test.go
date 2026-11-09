package ownership

import (
	"testing"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
)

func TestNames(t *testing.T) {
	for name, f := range map[string]func(string) bool{
		"Dockerfile":         isDockerfile,
		"Containerfile.dev":  isDockerfile,
		"app.Dockerfile":     isDockerfile,
		"Dockerfile.go":      isDockerfile,
		"CMakeLists.txt":     isCMake,
		"toolchain.cmake":    isCMake,
		"main.bicep":         isBicep,
		"prod.bicepparam":    isBicep,
		"guide.adoc":         isAsciiDoc,
		"guide.asciidoc":     isAsciiDoc,
	} {
		if !f(name) {
			t.Errorf("%s should match", name)
		}
	}
	for name, f := range map[string]func(string) bool{
		"dockerfile_notes.md": isDockerfile,
		"cmakelists.txt":      isCMake,
		"main.bicep.json":     isBicep,
		"key.asc":             isAsciiDoc,
	} {
		if f(name) {
			t.Errorf("%s should not match", name)
		}
	}
}

func TestOCIStale(t *testing.T) {
	images := []string{"app"}
	wrapper := func(name, image string) *rule.Rule {
		r := rule.NewRule(ociQualityKind, name)
		r.SetAttr("image", image)
		return r
	}
	if ociStale(wrapper("app_quality", ":app"), images) {
		t.Error("wrapper of a declared image should not be stale")
	}
	if !ociStale(wrapper("gone_quality", ":gone"), images) {
		t.Error("wrapper of a removed image should be stale")
	}
	if ociStale(wrapper("gone_quality", ":other"), images) {
		t.Error("wrapper of another target should not be stale")
	}
	if ociStale(wrapper("app", ":app"), images) {
		t.Error("rule without the quality suffix should not be stale")
	}
}
