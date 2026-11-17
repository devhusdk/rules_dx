package swift

import (
	"path/filepath"
	"slices"
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
)

const (
	IOSAppKind   = "apple_ios_app"
	MacOSAppKind = "apple_macos_app"
	appleDefs    = "apple:defs.bzl"
	iosPlatform  = "@rules_dx//apple:ios_sim_arm64"
	appVersion   = "1.0"
)

var appKindInfo = rule.KindInfo{NonEmptyAttrs: map[string]bool{"entry": true}}

// AppKind returns the Apple app kind an @main source that imports UIKit or
// AppKit names, or "" for any other source.
func AppKind(content []byte) string {
	if !DefinesMain(content) {
		return ""
	}
	imports := ParseImports(content)
	ios := slices.Contains(imports, "UIKit")
	macos := slices.Contains(imports, "AppKit")
	switch {
	case ios && !macos:
		return IOSAppKind
	case macos && !ios:
		return MacOSAppKind
	}
	return ""
}

// appEntries returns the library sources in dir that hold an Apple app's
// @main, and the one app kind they name, or nil when they name none or both.
func appEntries(dir string, files []string) (map[string]bool, string) {
	entries := make(map[string]bool)
	kinds := make(map[string]bool)
	for _, file := range files {
		if !isSupported(file) || IsTestSource(file) || IsEntryFile(file) {
			continue
		}
		content, err := common.ReadSource(filepath.Join(dir, file))
		if err != nil {
			continue
		}
		if kind := AppKind(content); kind != "" {
			entries[file] = true
			kinds[kind] = true
		}
	}
	if len(kinds) != 1 {
		return nil, ""
	}
	for kind := range kinds {
		return entries, kind
	}
	return nil, ""
}

// BundleID returns the starter bundle identifier of the app over library.
func BundleID(library string) string {
	return "com.example." + strings.ReplaceAll(library, "_", "-")
}

// appRule returns the app of kind whose entry is the library named library.
func appRule(kind, library string) *rule.Rule {
	r := rule.NewRule(kind, library+"_app")
	r.SetAttr("entry", ":"+library)
	if kind == IOSAppKind {
		r.SetAttr("platform", iosPlatform)
	}
	r.SetAttr("bundle_id", BundleID(library))
	r.SetAttr("version", appVersion)
	return r
}
