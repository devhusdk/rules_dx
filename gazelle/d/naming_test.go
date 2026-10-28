package d

import (
	"strings"
	"testing"
)

func TestImportRoot(t *testing.T) {
	tests := []struct {
		name, pkg, file, declared, want, wantErr string
	}{
		{"flat no declaration", "tools/x", "util.d", "", ".", ""},
		{"flat declared", "tools/x", "util.d", "util", ".", ""},
		{"src layout", "src/app/cli", "main.d", "app.cli.main", "../..", ""},
		{"deeper", "src/app/cli", "x.d", "app.cli.x", "../..", ""},
		{"one level", "src/app", "x.d", "app.x", "..", ""},
		{"package d", "src/app/cli", "package.d", "app.cli", "../..", ""},
		{"package d one segment", "src/app", "package.d", "app", "..", ""},
		{"interface file", "src/app", "x.di", "app.x", "..", ""},
		{"repo root", "", "demo.d", "demo", ".", ""},
		{"wrong stem", "src/app", "x.d", "app.y", "", "does not match its path"},
		{"wrong dir", "src/app", "x.d", "lib.x", "", "does not match its path"},
		{"misaligned", "src/myapp", "x.d", "app.x", "", "does not match its path"},
		{"too deep", "app", "x.d", "a.b.app.x", "", "does not match its path"},
		{"bad stem", "src", "my-file.d", "", "", "not a D identifier"},
		{"package d at root", "", "package.d", "", "", "repo root"},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got, err := ImportRoot(tc.pkg, tc.file, tc.declared)
			if tc.wantErr != "" {
				if err == nil || !strings.Contains(err.Error(), tc.wantErr) {
					t.Fatalf("ImportRoot error = %v, want %q", err, tc.wantErr)
				}
				return
			}
			if err != nil || got != tc.want {
				t.Fatalf("ImportRoot = %q, %v, want %q", got, err, tc.want)
			}
		})
	}
}

func TestModuleOfPackageWithoutDeclaration(t *testing.T) {
	got, err := ModuleOf("src/app", "package.d", "")
	if err != nil || got != "app" {
		t.Fatalf("ModuleOf = %q, %v, want app", got, err)
	}
}

func TestPackageRoot(t *testing.T) {
	root, err := PackageRoot("src/app/cli", map[string]string{"a.d": "app.cli.a", "b.d": "app.cli.b"})
	if err != nil || root != "../.." {
		t.Fatalf("PackageRoot = %q, %v", root, err)
	}
	_, err = PackageRoot("src/app", map[string]string{"a.d": "app.a", "b.d": ""})
	if err == nil || !strings.Contains(err.Error(), "b.d needs import root") || !strings.Contains(err.Error(), "a.d") {
		t.Fatalf("mixed roots error = %v, want both files named", err)
	}
	_, err = PackageRoot("src/app", map[string]string{"a.d": "app.a", "a.di": "app.a"})
	if err == nil || !strings.Contains(err.Error(), "a.d and a.di both define module app.a") {
		t.Fatalf("pair error = %v", err)
	}
}
