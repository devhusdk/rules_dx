package perl

import "testing"

func TestModuleName(t *testing.T) {
	cases := map[string]string{
		"lib/Inventory/Item.pm": "Inventory::Item",
		"lib/Top.pm":            "Top",
		"Top.pm":                "Top",
		"Sub/Mod.pm":            "Sub::Mod",
	}
	for src, want := range cases {
		if got := ModuleName(src); got != want {
			t.Errorf("ModuleName(%q) = %q, want %q", src, got, want)
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
