package cc

import (
	"path/filepath"
	"sort"
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/language"
)

// generateCuda emits the single Gazelle-owned cuda_library, <dir>_cuda, for
// .cu and .cuh files that no hand-written cuda_* rule lists. Hand-written
// targets are never matched, merged or deleted.
func generateCuda(args language.GenerateArgs, rep common.Reporter) language.GenerateResult {
	base, err := common.PackageTargetName(args)
	if err != nil {
		if hasCudaFiles(args.RegularFiles) {
			rep.Fail("cc: %s: %v", args.Rel, err)
		}
		return language.GenerateResult{}
	}
	name := base + "_cuda"

	owned := map[string]bool{}
	var existing *rule.Rule
	if args.File != nil {
		for _, r := range args.File.Rules {
			if !strings.HasPrefix(r.Kind(), "cuda_") {
				continue
			}
			if r.Name() == name {
				existing = r
				continue
			}
			for _, attr := range []string{"srcs", "hdrs"} {
				for _, f := range r.AttrStrings(attr) {
					owned[f] = true
				}
			}
		}
	}

	var srcs, hdrs []string
	for _, f := range args.RegularFiles {
		if IsTestSource(f) || owned[f] {
			continue
		}
		switch {
		case IsCudaSource(f):
			srcs = append(srcs, f)
		case IsCudaHeader(f):
			hdrs = append(hdrs, f)
		}
	}
	sort.Strings(srcs)
	sort.Strings(hdrs)

	if len(srcs) == 0 && len(hdrs) == 0 {
		if existing != nil && existing.Kind() == CudaKind {
			return language.GenerateResult{Empty: []*rule.Rule{rule.NewRule(CudaKind, name)}}
		}
		return language.GenerateResult{}
	}

	var archs []string
	if args.Config != nil {
		archs, _ = args.Config.Exts[archsDirective].([]string)
	}
	if len(archs) == 0 {
		rep.Fail("cc: %s: .cu/.cuh files need # gazelle:%s <arch>...", args.Rel, archsDirective)
		return language.GenerateResult{}
	}

	seen := map[string]bool{}
	var imports []string
	for _, f := range append(append([]string{}, srcs...), hdrs...) {
		content, err := common.ReadSource(filepath.Join(args.Dir, f))
		if err != nil {
			rep.Fail("cc: %s: read %s: %v", args.Rel, f, err)
			continue
		}
		if IsCudaSource(f) && DefinesMain(content) {
			rep.Fail("cc: %s: %s defines main; thin cuda_binary entries stay handwritten, so split main-defining sources into their own directory before adopting generation", args.Rel, f)
			continue
		}
		for _, inc := range ParseQuotedIncludes(content) {
			if !seen[inc] {
				seen[inc] = true
				imports = append(imports, inc)
			}
		}
	}
	if rep.Failed() {
		return language.GenerateResult{}
	}
	sort.Strings(imports)

	if args.File != nil {
		for _, r := range args.File.Rules {
			switch r.Kind() {
			case "cuda_binary", "cuda_test", "cuda_gpu_test":
				if r.Name() == base {
					rep.Fail("cc: %s: %s(%s) already creates %s; rename it or the generated target", args.Rel, r.Kind(), r.Name(), name)
					return language.GenerateResult{}
				}
			}
		}
	}
	if err := common.CheckClaimsDir(args.File, args.OtherGen, []common.Claimant{{Name: name, Source: args.Rel, Kind: CudaKind}}, CudaKind); err != nil {
		rep.Fail("cc: %s: %v", args.Rel, err)
		return language.GenerateResult{}
	}

	r := rule.NewRule(CudaKind, name)
	if len(srcs) > 0 {
		r.SetAttr("srcs", srcs)
	}
	if len(hdrs) > 0 {
		r.SetAttr("hdrs", hdrs)
	}
	r.SetAttr("archs", archs)
	if common.IsFixturePath(args.Rel) {
		r.SetAttr("testonly", true)
	}
	return language.GenerateResult{Gen: []*rule.Rule{r}, Imports: []any{common.WrapImportSet(imports)}}
}

func hasCudaFiles(files []string) bool {
	for _, f := range files {
		if IsCudaSource(f) || IsCudaHeader(f) {
			return true
		}
	}
	return false
}
