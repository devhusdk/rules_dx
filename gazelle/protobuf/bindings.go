package protobuf

import (
	"os"
	"path/filepath"
	"regexp"
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
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
		attrs := map[string]bool{"deps": true, "visibility": true, "testonly": true}
		attr := "deps"
		if b.kind == goKind {
			attrs = map[string]bool{"proto": true, "importpath": true, "deps": true, "testonly": true}
			attr = "proto"
		}
		info := rule.KindInfo{
			MatchAttrs:     []string{attr},
			NonEmptyAttrs:  map[string]bool{attr: true},
			MergeableAttrs: attrs,
		}
		if b.kind == goKind {
			info.ResolveAttrs = map[string]bool{"deps": true}
		}
		kinds[b.kind] = info
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
			var imp any
			if b.kind == goKind {
				path := goImportPath(args.Dir, lib.AttrStrings("srcs"))
				if path == "" {
					continue
				}
				r.SetAttr("proto", ":"+lib.Name())
				r.SetAttr("importpath", path)
				imp = common.WrapImportSet(libImports(result, lib))
			} else {
				r.SetAttr("deps", []string{":" + lib.Name()})
			}
			if lib.AttrBool("testonly") {
				r.SetAttr("testonly", true)
			}
			r.SetAttr("visibility", []string{"//visibility:public"})
			gen = append(gen, r)
			imports = append(imports, imp)
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

// libImports returns the import set Generate recorded for lib.
func libImports(result language.GenerateResult, lib *rule.Rule) []string {
	for i, r := range result.Gen {
		if r == lib && i < len(result.Imports) {
			if names, ok := common.UnwrapImportSet(result.Imports[i]); ok {
				return names
			}
		}
	}
	return nil
}

var (
	protoPackage = regexp.MustCompile(`(?m)^\s*package\s+([A-Za-z0-9_.]+)\s*;`)
	javaPackage  = regexp.MustCompile(`(?m)^\s*option\s+java_package\s*=\s*(?:"([^"\n]*)"|'([^'\n]*)')\s*;`)
)

// JavaPackage returns the java_package option, or else the proto package. It
// is empty when neither is set.
func JavaPackage(content []byte) string {
	content = stripComments(content)
	if m := javaPackage.FindSubmatch(content); m != nil {
		return string(m[1]) + string(m[2])
	}
	if m := protoPackage.FindSubmatch(content); m != nil {
		return string(m[1])
	}
	return ""
}

var csharpNamespace = regexp.MustCompile(`(?m)^\s*option\s+csharp_namespace\s*=\s*(?:"([^"\n]*)"|'([^'\n]*)')\s*;`)

// CSharpNamespace returns the csharp_namespace option, or else the proto
// package with each segment in PascalCase. It is empty when neither is set.
func CSharpNamespace(content []byte) string {
	content = stripComments(content)
	if m := csharpNamespace.FindSubmatch(content); m != nil {
		return string(m[1]) + string(m[2])
	}
	m := protoPackage.FindSubmatch(content)
	if m == nil {
		return ""
	}
	parts := strings.Split(string(m[1]), ".")
	for i, part := range parts {
		parts[i] = pascalCase(part)
	}
	return strings.Join(parts, ".")
}

func pascalCase(name string) string {
	var out strings.Builder
	upper := true
	for _, ch := range name {
		switch {
		case ch == '_':
			upper = true
		case upper:
			out.WriteString(strings.ToUpper(string(ch)))
			upper = false
		default:
			out.WriteRune(ch)
			upper = ch >= '0' && ch <= '9'
		}
	}
	return out.String()
}

// Binding keys are indexed under the protobuf language as "<key>:<value>", so
// other languages find the binding of the code they import.
const (
	keyGo      = "go"
	keyGoProto = "go_proto"
	keyCSProto = "cs_proto"
	keyPython  = "python"
	keyJava    = "java"
	keyCC      = "cc"
)

// crossKeys maps the language of an importing rule to its binding key.
var crossKeys = map[string]string{"go": keyGo, "python": keyPython, "java": keyJava, "cc": keyCC}

func bindingKey(key, value string) string { return key + ":" + value }

func protoLabel(c *config.Config, pkg, name string) string {
	return label.New(c.RepoName, pkg, name).String()
}

// bindingImports indexes a binding rule under the imports of the code it
// generates.
func bindingImports(c *config.Config, r *rule.Rule, f *rule.File) []resolve.ImportSpec {
	if f == nil {
		return nil
	}
	var wrapped string
	if r.Kind() == goKind {
		wrapped = strings.TrimPrefix(r.AttrString("proto"), ":")
	} else if deps := r.AttrStrings("deps"); len(deps) == 1 {
		wrapped = strings.TrimPrefix(deps[0], ":")
	}
	var lib *rule.Rule
	for _, other := range f.Rules {
		if other.Kind() == LibraryKind && other.Name() == wrapped {
			lib = other
		}
	}
	if lib == nil {
		return nil
	}
	var specs []resolve.ImportSpec
	seen := map[string]bool{}
	add := func(key, value string) {
		imp := bindingKey(key, value)
		if value != "" && !seen[imp] {
			seen[imp] = true
			specs = append(specs, resolve.ImportSpec{Lang: languageName, Imp: imp})
		}
	}
	switch r.Kind() {
	case goKind:
		add(keyGo, r.AttrString("importpath"))
		add(keyGoProto, protoLabel(c, f.Pkg, lib.Name()))
		return specs
	}
	dir := filepath.Join(c.RepoRoot, f.Pkg)
	for _, src := range lib.AttrStrings("srcs") {
		stem := common.ModuleIdentity(f.Pkg, strings.TrimSuffix(src, filepath.Ext(src)))
		switch r.Kind() {
		case pyKind:
			add(keyPython, strings.ReplaceAll(stem, "/", ".")+"_pb2")
		case ccKind:
			add(keyCC, stem+".pb.h")
		case javaKind:
			if content, err := os.ReadFile(filepath.Join(dir, src)); err == nil {
				add(keyJava, JavaPackage(content))
			}
		}
	}
	return specs
}

// CrossResolve lets Go, Python, Java and C++ imports of generated proto code
// find the binding rule that produces it, and lets Rust imports of a prost
// crate find its rust_prost_library.
func (l *protoLang) CrossResolve(c *config.Config, ix *resolve.RuleIndex, imp resolve.ImportSpec, lang string) []resolve.FindResult {
	if (imp.Lang == "rust" || imp.Lang == "typescript" || imp.Lang == "csharp") && imp.Lang == lang {
		return ix.FindRulesByImport(imp, languageName)
	}
	key, ok := crossKeys[imp.Lang]
	if !ok {
		return nil
	}
	return ix.FindRulesByImportWithConfig(c, resolve.ImportSpec{Lang: languageName, Imp: bindingKey(key, imp.Imp)}, languageName)
}
