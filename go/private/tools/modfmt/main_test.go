package main

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

const dirty = "module   example.com/a\ngo 1.22\n"
const clean = "module example.com/a\n\ngo 1.22\n"

func write(t *testing.T, body string) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "go.mod")
	if err := os.WriteFile(path, []byte(body), 0o644); err != nil {
		t.Fatal(err)
	}
	return path
}

func TestCheckListsDirtyFileAndKeepsBytes(t *testing.T) {
	path := write(t, dirty)
	var out, errOut bytes.Buffer
	if code := run([]string{"-c", "-l", path}, &out, &errOut); code != 1 {
		t.Fatalf("code %d", code)
	}
	if strings.TrimSpace(out.String()) != path {
		t.Fatalf("listed %q", out.String())
	}
	if got, _ := os.ReadFile(path); string(got) != dirty {
		t.Fatalf("file changed: %q", got)
	}
}

func TestWriteFormatsThenCheckIsClean(t *testing.T) {
	path := write(t, dirty)
	var out, errOut bytes.Buffer
	if code := run([]string{"-w", path}, &out, &errOut); code != 0 {
		t.Fatalf("code %d", code)
	}
	if got, _ := os.ReadFile(path); string(got) != clean {
		t.Fatalf("got %q", got)
	}
	out.Reset()
	if code := run([]string{"-c", "-l", path}, &out, &errOut); code != 0 || out.Len() != 0 {
		t.Fatalf("code %d out %q", code, out.String())
	}
}

func TestMalformedIsAnError(t *testing.T) {
	path := write(t, "module (\n")
	var out, errOut bytes.Buffer
	if code := run([]string{"-c", "-l", path}, &out, &errOut); code != 2 {
		t.Fatalf("code %d", code)
	}
	if errOut.Len() == 0 {
		t.Fatal("no message")
	}
}
