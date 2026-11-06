package common

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func TestBuildSnapshotDanglingLink(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("creating symlinks needs privileges on Windows")
	}
	dir := t.TempDir()
	build := filepath.Join(dir, "BUILD.bazel")
	const target = "missing/BUILD.bazel"
	if err := os.Symlink(target, build); err != nil {
		t.Fatal(err)
	}
	var s BuildSnapshot
	if err := s.Record(build); err != nil {
		t.Fatal(err)
	}
	if err := s.Restore(); err != nil {
		t.Fatal(err)
	}
	if got, err := os.Readlink(build); err != nil || got != target {
		t.Fatalf("intact link = %q, %v; want %q", got, err, target)
	}

	if err := os.Remove(build); err != nil {
		t.Fatal(err)
	}
	if err := s.Restore(); err != nil {
		t.Fatal(err)
	}
	if got, err := os.Readlink(build); err != nil || got != target {
		t.Fatalf("removed link = %q, %v; want %q", got, err, target)
	}

	if err := os.Remove(build); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink("other/BUILD.bazel", build); err != nil {
		t.Fatal(err)
	}
	if err := s.Restore(); err != nil {
		t.Fatal(err)
	}
	if got, err := os.Readlink(build); err != nil || got != target {
		t.Fatalf("repointed link = %q, %v; want %q", got, err, target)
	}
}

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
	if err := s.RecordDir(dir, []string{"same", "mod", "gone", "made"}); err != nil {
		t.Fatal(err)
	}
	if err := s.Record(mod); err != nil {
		t.Fatal(err)
	}

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

func TestBuildSnapshotRecordErrors(t *testing.T) {
	if os.Geteuid() == 0 {
		t.Skip("permissions are not enforced for root")
	}
	dir := t.TempDir()
	locked := filepath.Join(dir, "locked")
	if err := os.Mkdir(locked, 0o755); err != nil {
		t.Fatal(err)
	}
	unreadable := filepath.Join(dir, "unreadable")
	if err := os.WriteFile(unreadable, []byte("keep"), 0o000); err != nil {
		t.Fatal(err)
	}
	hidden := filepath.Join(locked, "BUILD")
	if err := os.WriteFile(hidden, []byte("keep"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(locked, 0); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = os.Chmod(locked, 0o755) })

	var s BuildSnapshot
	for _, p := range []string{unreadable, hidden} {
		err := s.Record(p)
		if err == nil || !strings.Contains(err.Error(), p) {
			t.Fatalf("Record(%s) = %v, want error naming the file", p, err)
		}
	}
	if err := s.RecordDir(dir, []string{"unreadable"}); err == nil {
		t.Fatal("RecordDir must return the error")
	}
	if err := s.Restore(); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(locked, 0o755); err != nil {
		t.Fatal(err)
	}
	if got, err := os.ReadFile(hidden); err != nil || string(got) != "keep" {
		t.Fatalf("file removed or changed: %q, %v", got, err)
	}
}

func TestBuildSnapshotKeepsDirectory(t *testing.T) {
	dir := t.TempDir()
	build := filepath.Join(dir, "BUILD")
	if err := os.Mkdir(build, 0o755); err != nil {
		t.Fatal(err)
	}
	inner := filepath.Join(build, "file.txt")
	if err := os.WriteFile(inner, []byte("x"), 0o644); err != nil {
		t.Fatal(err)
	}
	var s BuildSnapshot
	if err := s.Record(build); err != nil {
		t.Fatal(err)
	}
	if err := s.Restore(); err != nil {
		t.Fatalf("restore: %v", err)
	}
	if _, err := os.Stat(inner); err != nil {
		t.Fatalf("directory contents lost: %v", err)
	}
	if err := os.Remove(inner); err != nil {
		t.Fatal(err)
	}
	if err := s.Restore(); err != nil {
		t.Fatalf("restore: %v", err)
	}
	if info, err := os.Stat(build); err != nil || !info.IsDir() {
		t.Fatalf("empty directory removed: %v", err)
	}
}
