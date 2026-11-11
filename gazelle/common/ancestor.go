package common

import (
	"fmt"
	"os"
	"path"
	"path/filepath"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
)

// AncestorOwner finds a rule in a package above rel whose srcs name a file
// under rel by relative path. It returns the rule's label, such as
// "//tests:mixed".
func AncestorOwner(c *config.Config, rel string) (string, bool) {
	for dir := path.Dir(rel); rel != "" && rel != "."; dir = path.Dir(dir) {
		if dir == "." {
			dir = ""
		}
		for _, name := range c.ValidBuildFileNames {
			file, err := rule.LoadFile(filepath.Join(c.RepoRoot, filepath.FromSlash(dir), name), dir)
			if err != nil {
				continue
			}
			for _, r := range file.Rules {
				for _, src := range r.AttrStrings("srcs") {
					if strings.HasPrefix(path.Join(dir, src), rel+"/") {
						return fmt.Sprintf("//%s:%s", dir, r.Name()), true
					}
				}
			}
			break
		}
		if dir == "" {
			break
		}
	}
	return "", false
}

// NoticeAncestorOwned reports on stderr that no BUILD file is created in rel
// because the ancestor rule owns files under it.
func NoticeAncestorOwned(rel, owner string) {
	fmt.Fprintf(os.Stderr, "{\"event\":\"notice\",\"code\":\"ancestor_owned\",\"package\":%q,\"rule\":%q}\n", "//"+rel, owner)
}
