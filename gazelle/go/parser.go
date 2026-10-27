package golang

import (
	"go/build/constraint"
	"go/parser"
	"go/token"
	"sort"
	"strconv"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
)

func ParseImports(content []byte) ([]string, error) {
	fset := token.NewFileSet()
	f, err := parser.ParseFile(fset, "source.go", content, parser.ImportsOnly)
	if err != nil {
		return nil, err
	}
	set := make(map[string]struct{})
	for _, imp := range f.Imports {
		raw, err := strconv.Unquote(imp.Path.Value)
		if err != nil || strings.TrimSpace(raw) == "" {
			continue
		}
		set[raw] = struct{}{}
	}
	out := make([]string, 0, len(set))
	for name := range set {
		out = append(out, name)
	}
	sort.Strings(out)
	return out, nil
}

func ParsePackage(content []byte) (string, error) {
	fset := token.NewFileSet()
	f, err := parser.ParseFile(fset, "source.go", content, parser.PackageClauseOnly)
	if err != nil {
		return "", err
	}
	return f.Name.Name, nil
}

// BuildExcluded reports whether the file's build constraint cannot hold without the ignore or tools tag.
func BuildExcluded(content []byte) (bool, error) {
	expr, err := buildConstraint(content)
	if err != nil || expr == nil {
		return false, err
	}
	return !satisfiable(expr, isBuildableTag), nil
}

func buildConstraint(content []byte) (constraint.Expr, error) {
	fset := token.NewFileSet()
	f, err := parser.ParseFile(fset, "source.go", content, parser.PackageClauseOnly|parser.ParseComments)
	if err != nil {
		return nil, err
	}
	var goBuild constraint.Expr
	var plusBuild []constraint.Expr
	for _, group := range f.Comments {
		if group.Pos() >= f.Package {
			break
		}
		for _, c := range group.List {
			switch {
			case constraint.IsGoBuild(c.Text):
				if goBuild != nil {
					continue
				}
				expr, err := constraint.Parse(c.Text)
				if err != nil {
					return nil, err
				}
				goBuild = expr
			case constraint.IsPlusBuild(c.Text):
				expr, err := constraint.Parse(c.Text)
				if err != nil {
					return nil, err
				}
				plusBuild = append(plusBuild, expr)
			}
		}
	}
	if goBuild != nil {
		return goBuild, nil
	}
	var out constraint.Expr
	for _, expr := range plusBuild {
		if out == nil {
			out = expr
			continue
		}
		out = &constraint.AndExpr{X: out, Y: expr}
	}
	return out, nil
}

const maxFreeTags = 10

var impliedOS = map[string]string{"android": "linux", "illumos": "solaris", "ios": "darwin"}

func satisfiable(expr constraint.Expr, free func(string) bool) bool {
	seen := make(map[string]bool)
	oses := []string{""}
	arches := []string{""}
	var tags []string
	expr.Eval(func(tag string) bool {
		if !free(tag) || seen[tag] {
			return false
		}
		seen[tag] = true
		switch {
		case rule.KnownOSSet[tag]:
			oses = append(oses, tag)
		case rule.KnownArchSet[tag]:
			arches = append(arches, tag)
		default:
			tags = append(tags, tag)
		}
		return false
	})
	if len(tags) > maxFreeTags {
		return true
	}
	for _, goos := range oses {
		for _, goarch := range arches {
			for mask := 0; mask < 1<<len(tags); mask++ {
				set := map[string]bool{goos: true, impliedOS[goos]: true, goarch: true}
				for i, tag := range tags {
					set[tag] = mask&(1<<i) != 0
				}
				if expr.Eval(func(tag string) bool { return tag != "" && set[tag] }) {
					return true
				}
			}
		}
	}
	return false
}

var neverBuiltTags = map[string]bool{"ignore": true, "tools": true}

func isBuildableTag(tag string) bool {
	return !neverBuiltTags[tag]
}

func IsCgoImport(path string) bool {
	return strings.TrimSpace(path) == "C"
}
