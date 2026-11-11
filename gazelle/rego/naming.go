package rego

import (
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"
)

var SupportedExts = []string{".rego"}

const LibraryKind = "rego_library"

// TestKind is the rule generated for each *_test.rego source.
const TestKind = "rego_test"

func IsTestSource(name string) bool { return strings.HasSuffix(name, "_test.rego") }

func isSupported(name string) bool { return common.HasExt(name, SupportedExts) }

func DirTargetName(dir string) (string, error) { return common.DirTargetName(dir) }

// Candidates returns the dotted prefixes of name, longest first, so an
// import of a rule or nested document finds the package that declares it.
func Candidates(name string) []string {
	var out []string
	for i := strings.LastIndexByte(name, '.'); i > 0; i = strings.LastIndexByte(name[:i], '.') {
		out = append(out, name[:i])
	}
	return out
}
