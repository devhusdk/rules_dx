package typescript

import "testing"

func TestIsStdLib(t *testing.T) {
	for _, name := range []string{"fs", "path", "node:fs", "node:path", "test", "node:test"} {
		if !IsStdLib(name) {
			t.Errorf("stdlib identity not recognized: %q", name)
		}
	}
	for _, name := range []string{"helper", "", "react", "fs-extra", "fs/promises/extra"} {
		if IsStdLib(name) {
			t.Errorf("non-stdlib recognized as stdlib: %q", name)
		}
	}
	if !IsStdLib("fs/promises") {
		t.Error("stdlib subpath fs/promises not recognized")
	}
}
