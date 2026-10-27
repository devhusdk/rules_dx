package common

import (
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
)

// TestSpec opts a generator into one test rule per test source. Generation
// leaves a test source alone when a handwritten test rule already lists it.
type TestSpec struct {
	// Kind is the generated test rule kind, for example java_test.
	Kind string
	// IsSource reports whether a directory file is a test source.
	IsSource func(name string) bool
	// Name gives the target name for a test source.
	Name func(src string) (string, error)
	// Make builds the rule for a test source. Nil gives Kind with srcs=[src].
	Make func(name, src string, content []byte) (*rule.Rule, error)
	// IsImplicit reports imports the test runner supplies, which never
	// become deps.
	IsImplicit func(imp string) bool
}

func (t *TestSpec) makeRule(name, src string, content []byte) (*rule.Rule, error) {
	if t.Make != nil {
		return t.Make(name, src, content)
	}
	r := rule.NewRule(t.Kind, name)
	r.SetAttr("srcs", []string{src})
	return r, nil
}

func isTestKind(t *TestSpec, kind string) bool {
	return kind == t.Kind || strings.HasSuffix(kind, "_test")
}

func isLocalTestSrc(src string) bool {
	return src != "" && !strings.ContainsAny(src, "/:\\@")
}

// generatedTest reports whether r is exactly the rule generation gives for
// its single local source.
func (t *TestSpec) generatedTest(r *rule.Rule) bool {
	if r.Kind() != t.Kind {
		return false
	}
	srcs := r.AttrStrings("srcs")
	if len(srcs) != 1 || !isLocalTestSrc(srcs[0]) || !t.IsSource(srcs[0]) {
		return false
	}
	name, err := t.Name(srcs[0])
	return err == nil && name == r.Name()
}

// testOwners returns the test sources that an existing test rule other than
// the generated one already lists. A test rule whose srcs is not a plain list
// (a glob, for example) owns every test source.
func testOwners(file *rule.File, other []*rule.Rule, t *TestSpec, tests []string) map[string]bool {
	isTest := make(map[string]bool, len(tests))
	for _, src := range tests {
		isTest[src] = true
	}
	var rules []*rule.Rule
	if file != nil {
		rules = append(rules, file.Rules...)
	}
	rules = append(rules, other...)
	owned := make(map[string]bool)
	for _, r := range rules {
		if !isTestKind(t, r.Kind()) || t.generatedTest(r) {
			continue
		}
		srcs := r.AttrStrings("srcs")
		if r.Attr("srcs") != nil && len(srcs) == 0 {
			for _, src := range tests {
				owned[src] = true
			}
			continue
		}
		for _, src := range srcs {
			src = strings.TrimPrefix(src, ":")
			if isTest[src] {
				owned[src] = true
			}
		}
	}
	return owned
}

// ownsGeneratedTest is the stale-sweep guard for generators with a TestSpec:
// a rule of the test kind is removed only when generation could have produced
// it, and every other kind keeps the plain sweep.
func ownsGeneratedTest(t *TestSpec) func(*rule.Rule) bool {
	if t == nil {
		return nil
	}
	return func(r *rule.Rule) bool {
		if r.Kind() != t.Kind {
			return true
		}
		return t.generatedTest(r)
	}
}
