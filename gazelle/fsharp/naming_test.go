package fsharp

import "testing"

func TestIsTestSource(t *testing.T) {
	if !IsTestSource("DemoTest.fs") || !IsTestSource("pkg/DemoTest.fs") {
		t.Fatal("IsTestSource missed a *Test.fs file")
	}
	for _, name := range []string{"Demo.fs", "TestHelper.fs", "Contest.fs", "DemoTest.fs.bak"} {
		if IsTestSource(name) {
			t.Errorf("IsTestSource(%q) = true, want false", name)
		}
	}
}

func TestDirTargetName(t *testing.T) {
	got, err := DirTargetName("pkg/demo-pkg")
	if err != nil || got != "demo_pkg" {
		t.Errorf("DirTargetName = %q, %v; want demo_pkg", got, err)
	}
}

func TestClassIdentity(t *testing.T) {
	if got := ClassIdentity("pkg/Demo.fs"); got != "Demo" {
		t.Errorf("ClassIdentity = %q, want Demo", got)
	}
}
