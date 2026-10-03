package astro

import "testing"

func TestTargetName(t *testing.T) {
	cases := []struct {
		name string
		want string
	}{
		{"demo.astro", "demo"},
		{"pkg/my-mod.astro", "my_mod"},
		{"Hello.astro", "Hello"},
		{"plain", "plain"},
	}
	for _, tc := range cases {
		if got, err := TargetName(tc.name); err != nil || got != tc.want {
			t.Errorf("TargetName(%q) = %q, %v; want %q", tc.name, got, err, tc.want)
		}
	}
	if _, err := TargetName("---.astro"); err == nil {
		t.Error("TargetName(---.astro) succeeded, want error")
	}
}

func TestModuleName(t *testing.T) {
	cases := []struct {
		name string
		want string
	}{
		{"demo.astro", "demo"},
		{"pkg/my-mod.astro", "my-mod"},
		{"Hello.astro", "Hello"},
		{"archive.tar.astro", "archive.tar"},
		{"plain", "plain"},
	}
	for _, tc := range cases {
		if got := ModuleName(tc.name); got != tc.want {
			t.Errorf("ModuleName(%q) = %q, want %q", tc.name, got, tc.want)
		}
	}
}
