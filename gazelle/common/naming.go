package common

import (
	"fmt"
	"path"
	"sort"
	"strings"
	"unicode"

	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
)

func Normalize(base string) (string, error) {
	var b strings.Builder
	b.Grow(len(base))
	pending := false
	for _, c := range base {
		if c == '_' || unicode.IsLetter(c) || unicode.IsDigit(c) {
			if pending && b.Len() > 0 {
				b.WriteByte('_')
			}
			pending = false
			b.WriteRune(c)
			continue
		}
		pending = true
	}
	out := b.String()
	if strings.Trim(out, "_") == "" {
		return "", fmt.Errorf("naming: %q normalizes to an empty target name", base)
	}
	return out, nil
}

func stripExt(base string, exts []string) (string, bool) {
	for _, ext := range exts {
		if strings.HasSuffix(base, ext) {
			return strings.TrimSuffix(base, ext), true
		}
	}
	return "", false
}

func DirTargetName(dir string) (string, error) {
	return Normalize(path.Base(dir))
}

func RootTargetName(hints ...string) string {
	for _, hint := range hints {
		if name, err := Normalize(hint); err == nil {
			return name
		}
	}
	return "lib"
}

func PackageTargetName(args language.GenerateArgs, hints ...string) (string, error) {
	if args.Rel != "" {
		return DirTargetName(args.Rel)
	}
	repoName := ""
	if args.Config != nil {
		repoName = args.Config.RepoName
	}
	return RootTargetName(append(append([]string{}, hints...), repoName)...), nil
}

func Stem(name string, exts []string) string {
	base := path.Base(name)
	if trimmed, ok := stripExt(base, exts); ok {
		return trimmed
	}
	return base
}

func IsEntryFile(name string, exts []string) bool {
	base := path.Base(name)
	for _, ext := range exts {
		if base == "main"+ext {
			return true
		}
	}
	return false
}

func BinaryName(target string) string {
	return target + "_bin"
}

func UnitTestName(target string) string {
	return target + "_test"
}

// SnakeName names a per-source target after the file stem in snake case:
// GreetTest.java gives greet_test and HTTPClientTest.java http_client_test.
func SnakeName(name string, exts []string) (string, error) {
	stem := Stem(name, exts)
	var b strings.Builder
	b.Grow(len(stem) + 4)
	isUpper := func(c byte) bool { return c >= 'A' && c <= 'Z' }
	isLowerOrDigit := func(c byte) bool { return (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') }
	for i := 0; i < len(stem); i++ {
		c := stem[i]
		if isUpper(c) && i > 0 {
			prev := stem[i-1]
			nextLower := i+1 < len(stem) && stem[i+1] >= 'a' && stem[i+1] <= 'z'
			if isLowerOrDigit(prev) || (isUpper(prev) && nextLower) {
				b.WriteByte('_')
			}
		}
		if isUpper(c) {
			c += 'a' - 'A'
		}
		b.WriteByte(c)
	}
	return Normalize(b.String())
}

func ClassIdentity(name string, exts []string) string {
	short := Stem(name, exts)
	if i := strings.LastIndex(short, "."); i >= 0 {
		return short[i+1:]
	}
	return short
}

func IsTestSource(name string, exts []string, suffixes []string) bool {
	short := Stem(name, exts)
	for _, suffix := range suffixes {
		if strings.HasSuffix(short, suffix) {
			return true
		}
	}
	return false
}

func SuffixTestName(name string, exts, suffixes []string) (string, error) {
	stem := Stem(name, exts)
	for _, suffix := range suffixes {
		if strings.HasSuffix(stem, suffix) {
			stem = strings.TrimSuffix(stem, suffix)
			break
		}
	}
	return Normalize(stem + "_test")
}

type Claimant struct {
	Name   string
	Source string
	Kind   string
}

type CollisionError struct {
	Name      string
	Claimants []string
}

func (e *CollisionError) Error() string {
	return fmt.Sprintf("naming: normalized name %q claimed by %s; rename a source or keep one target handwritten",
		e.Name, strings.Join(e.Claimants, ", "))
}

func UnionStrings(a, b []string) []string {
	seen := make(map[string]bool, len(a)+len(b))
	var out []string
	for _, s := range append(append([]string{}, a...), b...) {
		if !seen[s] {
			seen[s] = true
			out = append(out, s)
		}
	}
	sort.Strings(out)
	return out
}

// FixtureTestonlyDirective opts a repository into marking libraries under
// tests/, fixtures/ or testdata/ testonly. Only rules_dx sets it.
const FixtureTestonlyDirective = "dx_fixture_testonly"

func SetFixtureTestonly(c *config.Config, on bool) {
	if c.Exts == nil {
		c.Exts = map[string]interface{}{}
	}
	c.Exts[FixtureTestonlyDirective] = on
}

func FixtureTestonly(c *config.Config, rel string) bool {
	if c == nil {
		return false
	}
	on, _ := c.Exts[FixtureTestonlyDirective].(bool)
	return on && IsFixturePath(rel)
}

func IsFixturePath(rel string) bool {
	padded := "/" + rel + "/"
	return strings.Contains(padded, "/tests/") || strings.Contains(padded, "/fixtures/") || strings.Contains(padded, "/testdata/")
}
