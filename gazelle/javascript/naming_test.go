package javascript

import "testing"

func TestIsTestFile(t *testing.T) {
	cases := []struct {
		name string
		want bool
	}{
		{"helper_test.js", true},
		{"pkg/helper_test.jsx", true},
		{"helper_test.mjs", true},
		{"helper_test.cjs", true},
		{"helper.js", false},
		{"test_helper.js", false},
		{"hello.test.js", false},
		{"tests/helper.js", false},
		{"helper_test.d.ts", false},
		{"helper_test", false},
		{"_test.js", true},
		{"helper_test.ts", false},
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
		{"demo.js", "demo"},
		{"pkg/my-mod.jsx", "my_mod"},
		{"helper_test.mjs", "helper_test"},
		{"entry.cjs", "entry"},
		{"plain", "plain"},
	}
	for _, tc := range cases {
		if got, err := TargetName(tc.name); err != nil || got != tc.want {
			t.Errorf("TargetName(%q) = %q, %v; want %q", tc.name, got, err, tc.want)
		}
	}
	if _, err := TargetName("---.js"); err == nil {
		t.Error("TargetName(---.js) succeeded, want error")
	}
}

func TestModuleName(t *testing.T) {
	cases := []struct {
		name string
		want string
	}{
		{"demo.js", "demo"},
		{"pkg/my-mod.jsx", "my-mod"},
		{"entry.mjs", "entry"},
		{"plain", "plain"},
		{"archive.tar.js", "archive.tar"},
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
		{"main.js", true},
		{"pkg/main.jsx", true},
		{"main.mjs", true},
		{"main.cjs", true},
		{"main_test.js", false},
		{"helper.js", false},
		{"main.ts", false},
		{"main", false},
		{"index.js", false},
		{"app.jsx", false},
		{"cli.mjs", false},
		{"pkg/index.cjs", false},
		{"bin.js", false},
		{"src/app.js", false},
	}
	for _, tc := range cases {
		if got := IsEntryFile(tc.name); got != tc.want {
			t.Errorf("IsEntryFile(%q) = %v, want %v", tc.name, got, tc.want)
		}
	}
}
