package perl

import "testing"

func TestModuleName(t *testing.T) {
	cases := []struct{ pkg, src, want string }{
		{"", "lib/Inventory/Item.pm", "Inventory::Item"},
		{"app", "lib/Top.pm", "Top"},
		{"app", "Top.pm", "Top"},
		{"app", "Sub/Mod.pm", "Sub::Mod"},
		{"inventory/lib/Inventory/Plugin", "Base.pm", "Inventory::Plugin::Base"},
		{"pkg/lib/Acme", "Widget.pm", "Acme::Widget"},
	}
	for _, c := range cases {
		if got := ModuleName(c.pkg, c.src); got != c.want {
			t.Errorf("ModuleName(%q, %q) = %q, want %q", c.pkg, c.src, got, c.want)
		}
	}
}

func TestTestTargetName(t *testing.T) {
	got, err := TestTargetName("t/item.t")
	if err != nil || got != "item_test" {
		t.Errorf("TestTargetName = %q, %v, want item_test", got, err)
	}
}

func TestIsStdLib(t *testing.T) {
	for name, want := range map[string]bool{"strict": true, "Test::More": true, "File::Spec": true, "Text::CSV": false, "Inventory::Item": false} {
		if got := IsStdLib(name); got != want {
			t.Errorf("IsStdLib(%q) = %v, want %v", name, got, want)
		}
	}
}
