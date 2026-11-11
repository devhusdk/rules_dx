package ownership

import (
	"testing"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
)

func TestNames(t *testing.T) {
	for name, f := range map[string]func(string) bool{
		"CMakeLists.txt":  isCMake,
		"toolchain.cmake": isCMake,
		"main.bicep":      isBicep,
		"prod.bicepparam": isBicep,
		"guide.adoc":      isAsciiDoc,
		"guide.asciidoc":  isAsciiDoc,
		"guide.rst":       isRst,
		"guide.rest":      isRst,
		"query.sql":       isSQL,
	} {
		if !f(name) {
			t.Errorf("%s should match", name)
		}
	}
	for name, f := range map[string]func(string) bool{
		"cmakelists.txt":  isCMake,
		"main.bicep.json": isBicep,
		"key.asc":         isAsciiDoc,
		"guide.rst.txt":   isRst,
		"query.sql.gz":    isSQL,
	} {
		if f(name) {
			t.Errorf("%s should not match", name)
		}
	}
}

// The lists mirror _DOCKERFILE_ACCEPTED and _DOCKERFILE_REJECTED in core/quality/sources_tests.bzl.
func TestDockerfileNames(t *testing.T) {
	for _, name := range []string{
		"Dockerfile",
		"Containerfile",
		"Dockerfile.dev",
		"Dockerfile.go",
		"Dockerfile.prebuilt",
		"Containerfile.ci",
		"app.Dockerfile",
		"app.dockerfile",
		"app.Containerfile",
		"app.containerfile",
	} {
		if !isDockerfile(name) {
			t.Errorf("%s should match", name)
		}
	}
	for _, name := range []string{
		"Dockerfile.dockerignore",
		"app.dockerignore",
		".dockerignore",
		"Dockerfile-dev",
		"dockerfile",
		"Dockerfile.",
		".Dockerfile",
		"Dockerfiles",
		"README.md",
		"dockerfile.md",
		"containerfile.md",
		"dockerfile_notes.md",
		"app.DOCKERFILE",
	} {
		if isDockerfile(name) {
			t.Errorf("%s should not match", name)
		}
	}
}

func TestYAMLSniff(t *testing.T) {
	for _, c := range []struct {
		head string
		f    func([]byte) bool
		want bool
	}{
		{"---\n- name: Site\n  hosts: all\n", isAnsiblePlaybook, true},
		{"- hosts: web\n", isAnsiblePlaybook, true},
		{"- name: task\n  debug:\n    msg: x\n", isAnsiblePlaybook, false},
		{"key: value\n", isAnsiblePlaybook, false},
		{"AWSTemplateFormatVersion: \"2010-09-09\"\n", isCloudFormation, true},
		{"# c\nResources:\n  B:\n", isCloudFormation, true},
		{"  Resources:\n", isCloudFormation, false},
		{"key: value\n", isCloudFormation, false},
	} {
		if got := c.f([]byte(c.head)); got != c.want {
			t.Errorf("%q: got %v, want %v", c.head, got, c.want)
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
