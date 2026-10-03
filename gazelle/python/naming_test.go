package python

import "testing"

func TestIsTestFile(t *testing.T) {
	cases := []struct {
		name string
		want bool
	}{
		{"helper_test.py", true},
		{"pkg/helper_test.py", true},
		{"helper.py", false},
		{"test_helper.py", false},
		{"tests/helper.py", false},
		{"helper_test.pyi", false},
		{"helper_test", false},
		{"_test.py", true},
	}
	for _, tc := range cases {
		if got := IsTestFile(tc.name); got != tc.want {
			t.Errorf("IsTestFile(%q) = %v, want %v", tc.name, got, tc.want)
		}
	}
}

func TestTargetName(t *testing.T) {
	cases := []struct {
		name string
		want string
	}{
		{"demo.py", "demo"},
		{"pkg/my-mod.py", "my_mod"},
		{"helper_test.py", "helper_test"},
		{"stub.pyi", "stub"},
		{"plain", "plain"},
	}
	for _, tc := range cases {
		if got, err := TargetName(tc.name); err != nil || got != tc.want {
			t.Errorf("TargetName(%q) = %q, %v; want %q", tc.name, got, err, tc.want)
		}
	}
	if _, err := TargetName("---.py"); err == nil {
		t.Error("TargetName(---.py) succeeded, want error")
	}
}

func TestModuleName(t *testing.T) {
	cases := []struct {
		name string
		want string
	}{
		{"demo.py", "demo"},
		{"pkg/my-mod.py", "my-mod"},
		{"stub.pyi", "stub"},
		{"plain", "plain"},
		{"archive.tar.gz", "archive.tar.gz"},
	}
	for _, tc := range cases {
		if got := ModuleName(tc.name); got != tc.want {
			t.Errorf("ModuleName(%q) = %q, want %q", tc.name, got, tc.want)
		}
	}
}

func TestIsEntryFile(t *testing.T) {
	cases := []struct {
		name string
		want bool
	}{
		{"main.py", true},
		{"pkg/main.py", true},
		{"main_test.py", false},
		{"helper.py", false},
		{"__main__.py", false},
		{"main.pyi", false},
		{"main", false},
	}
	for _, tc := range cases {
		if got := IsEntryFile(tc.name); got != tc.want {
			t.Errorf("IsEntryFile(%q) = %v, want %v", tc.name, got, tc.want)
		}
	}
}
