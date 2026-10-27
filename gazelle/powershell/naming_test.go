package powershell

import (
	"testing"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
)

func TestIsModuleSource(t *testing.T) {
	present := map[string]bool{"Greet.psm1": true, "Greet.psd1": true, "PSGallery.requirements.psd1": true}
	for name, want := range map[string]bool{
		"Greet.psm1":                  true,
		"Greet.psd1":                  true,
		"PSGallery.requirements.psd1": false,
		"Hello.ps1":                   false,
		"Greet.Tests.ps1":             false,
	} {
		if got := IsModuleSource(name, present); got != want {
			t.Errorf("IsModuleSource(%q) = %v, want %v", name, got, want)
		}
	}
}

func TestModuleName(t *testing.T) {
	for in, want := range map[string]string{
		"Greet.psm1":           "Greet",
		"Greet.psd1":           "Greet",
		"lib/My.Module.psm1":   "My.Module",
		"@repo//:Gallery.psd1": "Gallery",
		"//pkg:Other.psm1":     "Other",
	} {
		if got := ModuleName(in); got != want {
			t.Errorf("ModuleName(%q) = %q, want %q", in, got, want)
		}
	}
}

func TestTestTargetName(t *testing.T) {
	for in, want := range map[string]string{
		"Greet.Tests.ps1":     "greet_test",
		"My-Module.Tests.ps1": "my_module_test",
	} {
		if got, err := TestTargetName(in); err != nil || got != want {
			t.Errorf("TestTargetName(%q) = %q, %v; want %q", in, got, err, want)
		}
	}
	if _, err := TestTargetName("---.Tests.ps1"); err == nil {
		t.Error("TestTargetName(---.Tests.ps1) succeeded, want error")
	}
	if IsTestSource(".Tests.ps1") || !IsTestSource("A.Tests.ps1") || IsTestSource("pack_test.ps1") {
		t.Error("IsTestSource misclassifies")
	}
}

func TestOwns(t *testing.T) {
	mk := func(kind string, srcs ...string) *rule.Rule {
		r := rule.NewRule(kind, "x")
		if srcs != nil {
			r.SetAttr("srcs", srcs)
		}
		return r
	}
	cases := []struct {
		r    *rule.Rule
		want bool
	}{
		{mk(LibraryKind, "Greet.psd1", "Greet.psm1"), true},
		{mk(LibraryKind, "Greet.psm1", "Helper.ps1"), false},
		{mk(LibraryKind, "sub/Greet.psm1"), false},
		{mk(LibraryKind, "//other:Greet.psm1"), false},
		{mk(LibraryKind), false},
		{mk(TestKind, "Greet.Tests.ps1"), true},
		{mk(TestKind, "pack_test.ps1"), false},
		{mk(TestKind, "consumers/msix/A.Tests.ps1"), false},
		{mk(TestKind, "A.Tests.ps1", "B.Tests.ps1"), false},
		{mk("pwsh_binary", "Hello.ps1"), false},
	}
	for _, tc := range cases {
		if got := Owns(tc.r); got != tc.want {
			t.Errorf("Owns(%s %v) = %v, want %v", tc.r.Kind(), tc.r.AttrStrings("srcs"), got, tc.want)
		}
	}
}

func TestIsStdLib(t *testing.T) {
	for _, name := range []string{"Microsoft.PowerShell.Utility", "microsoft.powershell.management", "PSReadLine", "ThreadJob", "CimCmdlets"} {
		if !IsStdLib(name) {
			t.Errorf("IsStdLib(%q) = false", name)
		}
	}
	for _, name := range []string{"Pester", "PSScriptAnalyzer", "Greet"} {
		if IsStdLib(name) {
			t.Errorf("IsStdLib(%q) = true", name)
		}
	}
}
