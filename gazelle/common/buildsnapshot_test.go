package common

import (
	"os"
	"path/filepath"
	"testing"
)

func TestBuildSnapshotRestore(t *testing.T) {
	dir := t.TempDir()
	same := filepath.Join(dir, "same")
	mod := filepath.Join(dir, "mod")
	gone := filepath.Join(dir, "gone")
	made := filepath.Join(dir, "made")
	for _, p := range []string{same, mod, gone} {
		if err := os.WriteFile(p, []byte("orig "+filepath.Base(p)), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	var s BuildSnapshot
	s.RecordDir(dir, []string{"same", "mod", "gone", "made"})
	s.Record(mod)

	if err := os.WriteFile(mod, []byte("changed"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.Remove(gone); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(made, []byte("new"), 0o644); err != nil {
		t.Fatal(err)
	}
	for i := 0; i < 2; i++ {
		if err := s.Restore(); err != nil {
			t.Fatalf("restore %d: %v", i, err)
		}
		for _, p := range []string{same, mod, gone} {
			got, err := os.ReadFile(p)
			if err != nil || string(got) != "orig "+filepath.Base(p) {
				t.Fatalf("%s = %q, %v", p, got, err)
			}
		}
		if _, err := os.Stat(made); !os.IsNotExist(err) {
			t.Fatalf("created file survived restore: %v", err)
		}
	}
	s.Reset()
	if err := os.WriteFile(made, []byte("x"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := s.Restore(); err != nil {
		t.Fatal(err)
	}
	if _, err := os.Stat(made); err != nil {
		t.Fatalf("reset snapshot must not touch files: %v", err)
	}
}
