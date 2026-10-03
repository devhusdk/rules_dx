package typescript

import "testing"

func TestIsDeclaration(t *testing.T) {
	for _, name := range []string{"foo.d.ts", "pkg/foo.d.mts", "foo.d.cts"} {
		if !IsDeclaration(name) {
			t.Errorf("declaration not recognized: %q", name)
		}
	}
	for _, name := range []string{"foo.ts", "foo.tsx", "foo.mts", "foo.cts", "foo.d.js", "d.ts"} {
		if IsDeclaration(name) {
			t.Errorf("non-declaration recognized: %q", name)
		}
	}
}

func TestIsTestFile(t *testing.T) {
	cases := []struct {
		name string
		want bool
	}{
		{"helper_test.ts", true},
		{"pkg/helper_test.tsx", true},
		{"helper_test.mts", true},
		{"helper_test.cts", true},
		{"helper.ts", false},
		{"helper.d.ts", false},
		{"helper_test.d.ts", false},
		{"helper.test.ts", false},
		{"helper_test.js", false},
		{"helper_test", false},
		{"_test.ts", true},
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
		{"demo.ts", "demo"},
		{"pkg/my-mod.tsx", "my_mod"},
		{"helper_test.mts", "helper_test"},
		{"entry.cts", "entry"},
		{"plain", "plain"},
		{"demo.js", "demo_js"},
	}
	for _, tc := range cases {
		if got, err := TargetName(tc.name); err != nil || got != tc.want {
			t.Errorf("TargetName(%q) = %q, %v; want %q", tc.name, got, err, tc.want)
		}
	}
	if _, err := TargetName("---.ts"); err == nil {
		t.Error("TargetName(---.ts) succeeded, want error")
	}
}

func TestModuleName(t *testing.T) {
	cases := []struct {
		name string
		want string
	}{
		{"demo.ts", "demo"},
		{"pkg/my-mod.tsx", "my-mod"},
		{"entry.mts", "entry"},
		{"plain", "plain"},
		{"demo.js", "demo.js"},
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
		{"main.ts", true},
		{"pkg/main.tsx", true},
		{"main.mts", true},
		{"main.cts", true},
		{"main_test.ts", false},
		{"main.d.ts", false},
		{"helper.ts", false},
		{"main.js", false},
		{"main", false},
		{"index.ts", false},
		{"app.ts", false},
		{"cli.tsx", false},
		{"pkg/index.mts", false},
		{"bin.cts", false},
		{"src/app.ts", false},
	}
	for _, tc := range cases {
		if got := IsEntryFile(tc.name); got != tc.want {
			t.Errorf("IsEntryFile(%q) = %v, want %v", tc.name, got, tc.want)
		}
	}
}

func TestEntryPointName(t *testing.T) {
	cases := []struct {
		name string
		want string
	}{
		{"main.ts", "main.js"},
		{"pkg/main.tsx", "main.js"},
		{"main.mts", "main.mjs"},
		{"main.cts", "main.cjs"},
		{"main.d.ts", "main.d.js"},
		{"pkg/notes.md", "notes.md"},
	}
	for _, tc := range cases {
		if got := EntryPointName(tc.name); got != tc.want {
			t.Errorf("EntryPointName(%q) = %q, want %q", tc.name, got, tc.want)
		}
	}
}
