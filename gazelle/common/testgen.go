package common

import (
	"slices"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
)

type TestSpec struct {
	Kind       string
	AltKinds   []string
	IsSource   func(name string) bool
	Name       func(src string) (string, error)
	Make       func(name, src string, content []byte) (*rule.Rule, error)
	IsImplicit func(imp string) bool
}

func SizeSmall(r *rule.Rule) {
	if r.Attr("size") == nil {
		r.SetAttr("size", "small")
	}
}

func (t *TestSpec) makeRule(name, src string, content []byte) (*rule.Rule, error) {
	if t.Make != nil {
		r, err := t.Make(name, src, content)
		if r != nil {
			SizeSmall(r)
		}
		return r, err
	}
	r := rule.NewRule(t.Kind, name)
	r.SetAttr("srcs", []string{src})
	SizeSmall(r)
	return r, nil
}

func (t *TestSpec) generatedKind(kind string) bool {
	return kind == t.Kind || slices.Contains(t.AltKinds, kind)
}

func isTestKind(t *TestSpec, kind string) bool {
	return kind == t.Kind || strings.HasSuffix(kind, "_test")
}

func isLocalTestSrc(src string) bool {
	return src != "" && !strings.ContainsAny(src, "/:\\@")
}

func (t *TestSpec) generatedTest(r *rule.Rule) bool {
	if !t.generatedKind(r.Kind()) {
		return false
	}
	srcs := r.AttrStrings("srcs")
	if len(srcs) != 1 || !isLocalTestSrc(srcs[0]) || !t.IsSource(srcs[0]) {
		return false
	}
	name, err := t.Name(srcs[0])
	return err == nil && name == r.Name()
}

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
		if r.Kind() == t.Kind && r.Attr("srcs") != nil && len(srcs) == 0 {
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

func handwrittenTestSources(file *rule.File, t *TestSpec) map[string]bool {
	named := make(map[string]bool)
	if file == nil || t == nil {
		return named
	}
	for _, r := range file.Rules {
		if !t.generatedKind(r.Kind()) || t.generatedTest(r) {
			continue
		}
		for _, src := range r.AttrStrings("srcs") {
			named[strings.TrimPrefix(src, ":")] = true
		}
	}
	return named
}

func ownsGeneratedTest(t *TestSpec) func(*rule.Rule) bool {
	if t == nil {
		return nil
	}
	return func(r *rule.Rule) bool {
		if !t.generatedKind(r.Kind()) {
			return true
		}
		return t.generatedTest(r)
	}
}
