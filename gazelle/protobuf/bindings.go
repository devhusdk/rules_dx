package protobuf

import (
	"os"
	"path/filepath"
	"regexp"
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/language"
)

const (
	goKind   = "go_proto_library"
	pyKind   = "py_proto_library"
	javaKind = "java_proto_library"
	ccKind   = "cc_proto_library"
)

// binding is a language rule that wraps a proto_library. It is generated when
// the consumer's MODULE.bazel depends on the module that provides the rules.
type binding struct {
	module string
	kind   string
	infix  string
	defs   string
}

var bindings = []binding{
	{"rules_go", goKind, "_go", "@rules_go//proto:def.bzl"},
	{"rules_python", pyKind, "_py", "@protobuf//bazel:py_proto_library.bzl"},
	{"rules_java", javaKind, "_java", "@protobuf//bazel:java_proto_library.bzl"},
	{"rules_cc", ccKind, "_cc", "@protobuf//bazel:cc_proto_library.bzl"},
}

func bindingKinds() map[string]rule.KindInfo {
	kinds := map[string]rule.KindInfo{}
	for _, b := range bindings {
		attrs := map[string]bool{"deps": true, "visibility": true}
		attr := "deps"
		if b.kind == goKind {
			attrs = map[string]bool{"proto": true, "importpath": true}
			attr = "proto"
		}
		kinds[b.kind] = rule.KindInfo{
			MatchAttrs:     []string{attr},
			NonEmptyAttrs:  map[string]bool{attr: true},
			MergeableAttrs: attrs,
		}
	}
	return kinds
}

func bindingKindDefs() map[string]string {
	defs := map[string]string{}
	for _, b := range bindings {
		defs[b.kind] = b.defs
	}
	return defs
}

func bindingExtraKinds() []string {
	var kinds []string
	for _, b := range bindings {
		kinds = append(kinds, b.kind)
	}
	return kinds
}

var goPackage = regexp.MustCompile(`(?m)^\s*option\s+go_package\s*=\s*(?:"([^"\n]*)"|'([^'\n]*)')\s*;`)

// GoImportPath returns the import path of the go_package option, without the
// optional ";name" suffix. It is empty when the option is missing.
func GoImportPath(content []byte) string {
	m := goPackage.FindSubmatch(stripComments(content))
	if m == nil {
		return ""
	}
	value := string(m[1]) + string(m[2])
	value, _, _ = strings.Cut(value, ";")
	return value
}

// addBindings appends the binding rules for each generated proto_library.
func addBindings(args language.GenerateArgs, result language.GenerateResult) language.GenerateResult {
	deps := common.ModuleDeps(args.Config.RepoRoot)
	gen, imports := result.Gen, result.Imports
	desired := map[string]bool{}
	var names []string
	for _, lib := range result.Gen {
		if lib.Kind() != LibraryKind {
			continue
		}
		base := strings.TrimSuffix(lib.Name(), suffix)
		for _, b := range bindings {
			if !deps[b.module] {
				continue
			}
			r := rule.NewRule(b.kind, base+b.infix+suffix)
			if b.kind == goKind {
				path := goImportPath(args.Dir, lib.AttrStrings("srcs"))
				if path == "" {
					continue
				}
				r.SetAttr("proto", ":"+lib.Name())
				r.SetAttr("importpath", path)
			} else {
				r.SetAttr("deps", []string{":" + lib.Name()})
			}
			r.SetAttr("visibility", []string{"//visibility:public"})
			gen = append(gen, r)
			imports = append(imports, nil)
			desired[r.Kind()+"\x00"+r.Name()] = true
		}
	}
	result.Gen, result.Imports = gen, imports
	if args.File == nil {
		return result
	}
	for _, lib := range append(append([]*rule.Rule{}, result.Gen...), result.Empty...) {
		if lib.Kind() == LibraryKind {
			names = append(names, strings.TrimSuffix(lib.Name(), suffix))
		}
	}
	for _, existing := range args.File.Rules {
		for _, b := range bindings {
			if existing.Kind() != b.kind || desired[b.kind+"\x00"+existing.Name()] {
				continue
			}
			for _, base := range names {
				if existing.Name() == base+b.infix+suffix {
					result.Empty = append(result.Empty, rule.NewRule(existing.Kind(), existing.Name()))
				}
			}
		}
	}
	return result
}

func goImportPath(dir string, srcs []string) string {
	for _, src := range srcs {
		content, err := os.ReadFile(filepath.Join(dir, src))
		if err != nil {
			continue
		}
		if path := GoImportPath(content); path != "" {
			return path
		}
	}
	return ""
}
