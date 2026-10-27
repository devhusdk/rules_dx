package cc

import (
	"sort"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
)

const androidPlatform = "@platforms//os:android"

// androidAttrs sets NDK linkopts, Android compatibility and alwayslink from the sources it reads.
func androidAttrs(r *rule.Rule, _ []string, contents map[string][]byte) error {
	names := make([]string, 0, len(contents))
	for name := range contents {
		names = append(names, name)
	}
	sort.Strings(names)
	libs := make(map[string]bool)
	unguarded := false
	entry := false
	for _, name := range names {
		content := contents[name]
		for _, inc := range ParseSystemIncludes(content) {
			lib, ndk := common.NDKLib(inc.Path)
			if !ndk {
				continue
			}
			if !inc.Guarded {
				unguarded = true
			}
			if lib != "" {
				libs["-l"+lib] = true
			}
		}
		if DefinesAndroidEntry(content) {
			entry = true
		}
	}
	linkopts := make([]string, 0, len(libs))
	for opt := range libs {
		linkopts = append(linkopts, opt)
	}
	sort.Strings(linkopts)
	switch {
	case unguarded:
		r.SetAttr("target_compatible_with", []string{androidPlatform})
		if len(linkopts) > 0 {
			r.SetAttr("linkopts", linkopts)
		}
	case len(linkopts) > 0:
		r.SetAttr("linkopts", rule.SelectStringListValue{
			androidPlatform:        linkopts,
			"//conditions:default": {},
		})
	}
	if entry {
		r.SetAttr("alwayslink", true)
	}
	return nil
}
