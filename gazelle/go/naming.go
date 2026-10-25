package golang

import (
	"path"
	"regexp"
	"strings"

	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/devhusdk/rules_dx/gazelle/common"
)

var SupportedExts = []string{".go"}

const LibraryKind = "go_library"

const TestKind = "go_test"

func IsTestSource(name string) bool {
	return strings.HasSuffix(path.Base(name), "_test.go")
}

func DirTargetName(dir string) (string, error) { return common.DirTargetName(dir) }

var majorVersion = regexp.MustCompile(`^v[0-9]+$`)

func goRootHint(args language.GenerateArgs) string {
	if args.Rel != "" || args.Config == nil {
		return ""
	}
	module, ok := goImportPath(args.Config.RepoRoot, args.Dir)
	if !ok {
		return ""
	}
	parts := strings.Split(module, "/")
	last := parts[len(parts)-1]
	if majorVersion.MatchString(last) && len(parts) > 1 {
		last = parts[len(parts)-2]
	}
	return last
}

type Claimant = common.Claimant
