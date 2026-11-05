package protobuf

import "testing"

func TestDirTargetName(t *testing.T) {
	got, err := DirTargetName("api/v1")
	if err != nil || got != "v1_proto" {
		t.Errorf("DirTargetName = %q, %v, want v1_proto", got, err)
	}
}

func TestWellKnownLabel(t *testing.T) {
	cases := map[string]string{
		"google/protobuf/timestamp.proto":       "@protobuf//:timestamp_proto",
		"google/protobuf/compiler/plugin.proto": "",
		"google/api/annotations.proto":          "",
		"pkg/x.proto":                           "",
	}
	for imp, want := range cases {
		got, ok := WellKnownLabel(imp)
		if got != want || ok != (want != "") {
			t.Errorf("WellKnownLabel(%q) = %q, %v, want %q", imp, got, ok, want)
		}
	}
}
