package java

import "testing"

func TestIsTestSource(t *testing.T) {
	if !IsTestSource("DemoTest.java") || !IsTestSource("pkg/DemoTest.java") {
		t.Fatal("IsTestSource missed a *Test.java file")
	}
	for _, name := range []string{"Demo.java", "TestHelper.java", "Contest.java", "DemoTest.java.bak"} {
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
	if got := ClassIdentity("pkg/Demo.java"); got != "Demo" {
		t.Errorf("ClassIdentity = %q, want Demo", got)
	}
}
