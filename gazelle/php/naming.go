package php

import (
	"path"
	"strings"
	"unicode"

	"github.com/devhusdk/rules_dx/gazelle/common"
)

var SupportedExts = []string{".php"}

const (
	LibraryKind = "php_library"
	TestKind    = "php_test"
	srcDir      = "src"
	testSuffix  = "Test.php"
)

// testDirs hold the package's tests.
var testDirs = []string{"tests", "test"}

func DirTargetName(dir string) (string, error) { return common.DirTargetName(dir) }

// TestTargetName names the php_test of a FooBarTest.php source: foo_bar_test.
func TestTargetName(src string) (string, error) {
	stem := strings.TrimSuffix(path.Base(src), testSuffix)
	return common.Normalize(snake(stem) + "_test")
}

func isTestSource(name string) bool {
	return strings.HasSuffix(name, testSuffix) && len(name) > len(testSuffix)
}

func snake(s string) string {
	runes := []rune(s)
	var b strings.Builder
	for i, r := range runes {
		if unicode.IsUpper(r) && i > 0 {
			prev := runes[i-1]
			nextLower := i+1 < len(runes) && unicode.IsLower(runes[i+1])
			if unicode.IsLower(prev) || unicode.IsDigit(prev) || (unicode.IsUpper(prev) && nextLower) {
				b.WriteByte('_')
			}
		}
		b.WriteRune(unicode.ToLower(r))
	}
	return b.String()
}
