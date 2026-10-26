package cc

import (
	"os"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
	"testing"
)

// readList reads a one-line Starlark list assignment such as
// `_CC_SRCS = [".c", ".cc"]` from the file named by envVar. It fails the test
// when the variable, file or list is missing or has an unexpected format.
func readList(t *testing.T, envVar, name string) []string {
	t.Helper()
	rel := os.Getenv(envVar)
	if rel == "" {
		t.Fatalf("env %s is not set", envVar)
	}
	root := os.Getenv("TEST_SRCDIR")
	if root == "" {
		root = os.Getenv("RUNFILES_DIR")
	}
	if root == "" {
		t.Fatalf("neither TEST_SRCDIR nor RUNFILES_DIR is set")
	}
	data, err := os.ReadFile(filepath.Join(root, rel))
	if err != nil {
		t.Fatalf("read %s: %v", rel, err)
	}
	re := regexp.MustCompile(`(?m)^` + regexp.QuoteMeta(name) + `\s*=\s*\[([^\]\n]*)\]\s*$`)
	m := re.FindSubmatch(data)
	if m == nil {
		t.Fatalf("%s: no one-line list assignment for %s; update this test with the new format", rel, name)
	}
	var out []string
	for _, item := range strings.Split(string(m[1]), ",") {
		item = strings.TrimSpace(item)
		if item == "" {
			continue
		}
		if len(item) < 2 || item[0] != '"' || item[len(item)-1] != '"' {
			t.Fatalf("%s: unexpected item %q in %s", rel, item, name)
		}
		out = append(out, item[1:len(item)-1])
	}
	if len(out) == 0 {
		t.Fatalf("%s: %s is empty", rel, name)
	}
	return out
}

func sorted(in []string) []string {
	out := append([]string(nil), in...)
	sort.Strings(out)
	return out
}

func sameSet(a, b []string) bool {
	a, b = sorted(a), sorted(b)
	if len(a) != len(b) {
		return false
	}
	for i := range a {
		if a[i] != b[i] {
			return false
		}
	}
	return true
}

func TestWrapperExtsMatchGazelle(t *testing.T) {
	ccSrcs := readList(t, "CC_DEFS_BZL", "_CC_SRCS")
	ccHdrs := readList(t, "CC_DEFS_BZL", "_CC_HDRS")
	cudaSrcs := readList(t, "CUDA_DEFS_BZL", "_CUDA_SRCS")

	if !sameSet(ccSrcs, SupportedSrcExts) {
		t.Errorf("_CC_SRCS %v differs from SupportedSrcExts %v", ccSrcs, SupportedSrcExts)
	}
	if !sameSet(ccHdrs, SupportedHdrExts) {
		t.Errorf("_CC_HDRS %v differs from SupportedHdrExts %v", ccHdrs, SupportedHdrExts)
	}
	for _, ext := range cudaSrcs {
		for _, list := range [][]string{SupportedSrcExts, SupportedHdrExts} {
			for _, g := range list {
				if ext == g {
					t.Errorf("CUDA extension %q overlaps Gazelle cc list %v", ext, list)
				}
			}
		}
	}
}
