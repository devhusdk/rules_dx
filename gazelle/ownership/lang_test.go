package ownership

import "testing"

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
