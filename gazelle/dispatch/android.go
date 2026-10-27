package dispatch

import (
	"os"
	"path"
	"path/filepath"
	"strconv"
	"strings"

	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/rule"
	bzl "github.com/bazelbuild/buildtools/build"

	"github.com/devhusdk/rules_dx/gazelle/common"
)

const androidApkKind = "rust_android_apk"

// checkAndroid cross-checks custom manifests of rust_android_apk rules against their literal attrs.
func checkAndroid(args language.GenerateArgs) []string {
	if args.File == nil {
		return nil
	}
	names := make(map[string]bool, len(args.File.Rules))
	for _, r := range args.File.Rules {
		names[r.Name()] = true
	}
	var errs []string
	for _, r := range args.File.Rules {
		if r.Kind() != androidApkKind {
			continue
		}
		rel, ok := localManifest(r, names)
		if !ok {
			continue
		}
		content, err := os.ReadFile(filepath.Join(args.Dir, filepath.FromSlash(rel)))
		if err != nil {
			continue
		}
		where := "android: //" + args.Rel + ":" + r.Name()
		m, err := common.ParseAndroidManifest(content)
		if err != nil {
			errs = append(errs, where+": manifest "+rel+": "+err.Error())
			continue
		}
		if pkg, ok := effectivePackage(r); ok && m.Package != "" && m.Package != pkg {
			errs = append(errs, where+": manifest "+rel+" declares package "+strconv.Quote(m.Package)+", want "+strconv.Quote(pkg)+"; set custom_package or fix the manifest")
		}
		if api, ok := literalInt(r.Attr("api_level")); ok && api != 0 && m.MinSDK != 0 && api != m.MinSDK {
			errs = append(errs, where+": manifest "+rel+" declares minSdkVersion "+strconv.Itoa(m.MinSDK)+", but api_level is "+strconv.Itoa(api))
		}
	}
	return errs
}

func localManifest(r *rule.Rule, names map[string]bool) (string, bool) {
	s, ok := r.Attr("manifest").(*bzl.StringExpr)
	if !ok {
		return "", false
	}
	v := s.Value
	if strings.HasPrefix(v, "//") || strings.HasPrefix(v, "@") || v == "" {
		return "", false
	}
	v = strings.TrimPrefix(v, ":")
	if strings.Contains(v, ":") || path.IsAbs(v) || names[v] {
		return "", false
	}
	return v, true
}

func effectivePackage(r *rule.Rule) (string, bool) {
	if e := r.Attr("custom_package"); e != nil {
		s, ok := e.(*bzl.StringExpr)
		if !ok {
			return "", false
		}
		if s.Value != "" {
			return s.Value, true
		}
	}
	s, ok := r.Attr("app_id").(*bzl.StringExpr)
	if !ok || s.Value == "" {
		return "", false
	}
	return s.Value, true
}

func literalInt(e bzl.Expr) (int, bool) {
	lit, ok := e.(*bzl.LiteralExpr)
	if !ok {
		return 0, false
	}
	n, err := strconv.Atoi(lit.Token)
	return n, err == nil
}
