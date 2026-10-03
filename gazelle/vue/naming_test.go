package vue

import "testing"

func TestTargetName(t *testing.T) {
	cases := []struct {
		name string
		want string
	}{
		{"demo.vue", "demo"},
		{"pkg/my-mod.vue", "my_mod"},
		{"Hello.vue", "Hello"},
		{"plain", "plain"},
	}
	for _, tc := range cases {
		if got, err := TargetName(tc.name); err != nil || got != tc.want {
			t.Errorf("TargetName(%q) = %q, %v; want %q", tc.name, got, err, tc.want)
		}
	}
	if _, err := TargetName("---.vue"); err == nil {
		t.Error("TargetName(---.vue) succeeded, want error")
	}
}

func TestModuleName(t *testing.T) {
	cases := []struct {
		name string
		want string
	}{
		{"demo.vue", "demo"},
		{"pkg/my-mod.vue", "my-mod"},
		{"Hello.vue", "Hello"},
		{"archive.tar.vue", "archive.tar"},
		{"plain", "plain"},
	}
	for _, tc := range cases {
		if got := ModuleName(tc.name); got != tc.want {
			t.Errorf("ModuleName(%q) = %q, want %q", tc.name, got, tc.want)
		}
	}
}
