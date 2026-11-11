package common

import (
	"fmt"
	"path"
	"path/filepath"
	"slices"
	"sort"
	"strings"
	"sync"

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

// AncestorOwnedNotice records that no BUILD file is created in Package
// because Rule owns files under it.
type AncestorOwnedNotice struct {
	Package string `json:"package"`
	Rule    string `json:"rule"`
}

var (
	ancestorMu      sync.Mutex
	ancestorNotices []AncestorOwnedNotice
)

// NoticeAncestorOwned records that no BUILD file is created in rel because
// the ancestor rule owns files under it.
func NoticeAncestorOwned(rel, owner string) {
	ancestorMu.Lock()
	defer ancestorMu.Unlock()
	ancestorNotices = append(ancestorNotices, AncestorOwnedNotice{Package: rel, Rule: owner})
}

// TakeAncestorOwned returns the recorded notices sorted by package and rule,
// without duplicates, and empties the record.
func TakeAncestorOwned() []AncestorOwnedNotice {
	ancestorMu.Lock()
	out := ancestorNotices
	ancestorNotices = nil
	ancestorMu.Unlock()
	sort.Slice(out, func(i, j int) bool {
		if out[i].Package != out[j].Package {
			return out[i].Package < out[j].Package
		}
		return out[i].Rule < out[j].Rule
	})
	return slices.Compact(out)
}
