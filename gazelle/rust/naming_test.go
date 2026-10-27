package rust

import "testing"

func TestDerivedNameErrors(t *testing.T) {
	if got, err := BuildScriptName("---"); err == nil {
		t.Errorf("BuildScriptName(\"---\") = %q, want error", got)
	}
}

func TestDerivedNames(t *testing.T) {
	if got := IntegrationTestName("login"); got != "login_test" {
		t.Errorf("IntegrationTestName(login) = %q", got)
	}
	if got := IntegrationTestName("login_test"); got != "login_test" {
		t.Errorf("IntegrationTestName(login_test) = %q", got)
	}
	if got, err := BuildScriptName("my-pkg"); err != nil || got != "my_pkg_build_script" {
		t.Errorf("BuildScriptName = %q, %v", got, err)
	}
}

func TestCrateName(t *testing.T) {
	for in, want := range map[string]string{"my-pkg": "my_pkg", "a--b": "a__b", "_x": "_x", "demo": "demo"} {
		if got, err := CrateName(in); err != nil || got != want {
			t.Errorf("CrateName(%q) = %q, %v, want %q", in, got, err, want)
		}
	}
	for _, in := range []string{"", "a.b", "café"} {
		if got, err := CrateName(in); err == nil {
			t.Errorf("CrateName(%q) = %q, want error", in, got)
		}
	}
}
