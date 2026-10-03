package svelte

import "testing"

func TestTargetName(t *testing.T) {
	cases := []struct {
		name string
		want string
	}{
		{"demo.svelte", "demo"},
		{"pkg/my-mod.svelte", "my_mod"},
		{"Hello.svelte", "Hello"},
		{"plain", "plain"},
	}
	for _, tc := range cases {
		if got, err := TargetName(tc.name); err != nil || got != tc.want {
			t.Errorf("TargetName(%q) = %q, %v; want %q", tc.name, got, err, tc.want)
		}
	}
	if _, err := TargetName("---.svelte"); err == nil {
		t.Error("TargetName(---.svelte) succeeded, want error")
	}
}

func TestModuleName(t *testing.T) {
	cases := []struct {
		name string
		want string
	}{
		{"demo.svelte", "demo"},
		{"pkg/my-mod.svelte", "my-mod"},
		{"Hello.svelte", "Hello"},
		{"archive.tar.svelte", "archive.tar"},
		{"plain", "plain"},
	}
	for _, tc := range cases {
		if got := ModuleName(tc.name); got != tc.want {
			t.Errorf("ModuleName(%q) = %q, want %q", tc.name, got, tc.want)
		}
	}
}
