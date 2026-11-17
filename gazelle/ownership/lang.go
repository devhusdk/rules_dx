package ownership

import (
	"encoding/xml"
	"io"
	"os"
	"path"
	"path/filepath"
	"regexp"
	"slices"
	"sort"
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/language"
)

const languageName = "ownership"

// The quality wrapper of an oci_image is named after the image with ociQualitySuffix.
// An appimage_package is named after its desktop entry with appimageSuffix.
const (
	appimageKind    = "appimage_package"
	appimageSuffix  = "_appimage"
	appimageVersion = "1.0"
)

// appimageNotices are the file names an appimage_package or msix_package takes as notice.
var appimageNotices = []string{"COPYING", "LICENSE", "LICENSE.md", "LICENSE.txt", "NOTICE", "NOTICE.md", "NOTICE.txt"}

// An msix_package is named after its binary with msixSuffix and reads
// msixManifest.
const (
	msixKind     = "msix_package"
	msixSuffix   = "_msix"
	msixManifest = "Package.appxmanifest"
)

const (
	ociImageKind     = "oci_image"
	ociQualityKind   = "oci_image_quality"
	ociQualitySuffix = "_quality"
)

type family struct {
	kind     string
	target   string
	defs     string
	isSource func(name string) bool
	// sniff, when set, must also accept the first sniffBytes of the file.
	sniff func(head []byte) bool
	// buildable, when set, must accept the directory's matching files together.
	buildable func(dir string, srcs []string) bool
}

const sniffBytes = 4096

var (
	ansiblePlaybook = regexp.MustCompile(`(?m)^(?:- +hosts:|  hosts:)`)
	cfnTemplate     = regexp.MustCompile(`(?m)^(?:AWSTemplateFormatVersion|Resources):`)
	apiSpecRoot     = regexp.MustCompile(`(?m)^(?:openapi|asyncapi):`)
	k8sAPIVersion   = regexp.MustCompile(`(?m)^apiVersion:`)
	k8sKind         = regexp.MustCompile(`(?m)^kind:`)
	fortranProgram  = regexp.MustCompile(`(?im)^[ \t]*program[ \t]+\w`)
	fortranUse      = regexp.MustCompile(`(?im)^[ \t]*use\b[ \t]*(,[ \t]*(\w+)[ \t]*::|::)?[ \t]*(\w+)`)
)

var fortranIntrinsicModules = []string{"iso_c_binding", "iso_fortran_env", "ieee_arithmetic", "ieee_exceptions", "ieee_features"}

var families = []family{
	{"dockerfile_library", "dockerfile", "dockerfile:defs.bzl", isDockerfile, nil, nil},
	{"cmake_library", "cmake", "cmake:defs.bzl", isCMake, nil, nil},
	{"bicep_library", "bicep", "bicep:defs.bzl", isBicep, nil, nil},
	{"asciidoc_library", "asciidoc", "asciidoc:defs.bzl", isAsciiDoc, nil, nil},
	{"ansible_library", "ansible", "ansible:defs.bzl", isYAML, isAnsiblePlaybook, nil},
	{"cloudformation_library", "cloudformation", "cloudformation:defs.bzl", isYAML, isCloudFormation, nil},
	{"rst_library", "rst", "rst:defs.bzl", isRst, nil, nil},
	{"sql_library", "sql", "sql:defs.bzl", isSQL, nil, nil},
	{"make_library", "make", "make:defs.bzl", isMake, nil, nil},
	{"terraform_library", "terraform", "terraform:defs.bzl", isTerraform, nil, nil},
	{"kubernetes_library", "kubernetes", "kubernetes:defs.bzl", isYAML, isKubernetes, nil},
	{"api_spec_library", "api_spec", "api_spec:defs.bzl", isYAML, isAPISpec, nil},
	{"helm_library", "helm", "helm:defs.bzl", isHelmChart, nil, nil},
	{"fortran_library", "fortran", "fortran:defs.bzl", isFortran, isFortranModule, fortranSelfContained},
}

// isDockerfile matches the names is_dockerfile_name in core/quality/sources.bzl accepts.
func isDockerfile(name string) bool {
	if strings.HasSuffix(name, ".dockerignore") {
		return false
	}
	for _, stem := range []string{"Dockerfile", "Containerfile"} {
		if name == stem || (strings.HasPrefix(name, stem+".") && len(name) > len(stem)+1) {
			return true
		}
	}
	for _, suffix := range []string{".Dockerfile", ".dockerfile", ".Containerfile", ".containerfile"} {
		if strings.HasSuffix(name, suffix) && len(name) > len(suffix) {
			return true
		}
	}
	return false
}

func isCMake(name string) bool {
	return name == "CMakeLists.txt" || strings.HasSuffix(name, ".cmake")
}

func isBicep(name string) bool {
	return strings.HasSuffix(name, ".bicep") || strings.HasSuffix(name, ".bicepparam")
}

func isAsciiDoc(name string) bool {
	return strings.HasSuffix(name, ".adoc") || strings.HasSuffix(name, ".asciidoc")
}

func isRst(name string) bool {
	return strings.HasSuffix(name, ".rst") || strings.HasSuffix(name, ".rest")
}

func isSQL(name string) bool {
	return strings.HasSuffix(name, ".sql")
}

func isMake(name string) bool {
	return name == "Makefile" || name == "makefile" || name == "GNUmakefile" || strings.HasSuffix(name, ".mk")
}

func isTerraform(name string) bool {
	return strings.HasSuffix(name, ".tf")
}

func isHelmChart(name string) bool {
	return name == "Chart.yaml"
}

// isFortran matches the file_extensions in fortran/lang.json.
func isFortran(name string) bool {
	for _, suffix := range []string{".f", ".f03", ".f08", ".f90", ".f95"} {
		if strings.HasSuffix(name, suffix) {
			return true
		}
	}
	return false
}

func isYAML(name string) bool {
	return strings.HasSuffix(name, ".yml") || strings.HasSuffix(name, ".yaml")
}

func isAnsiblePlaybook(head []byte) bool { return ansiblePlaybook.Match(head) }

func isKubernetes(head []byte) bool { return k8sAPIVersion.Match(head) && k8sKind.Match(head) }

func isAPISpec(head []byte) bool { return apiSpecRoot.Match(head) }

func isFortranModule(head []byte) bool { return !fortranProgram.Match(head) }

// fortranSelfContained reports whether every use statement in srcs names an
// intrinsic module.
func fortranSelfContained(dir string, srcs []string) bool {
	for _, name := range srcs {
		data, err := os.ReadFile(filepath.Join(dir, name))
		if err != nil || !fortranUsesOnlyIntrinsics(data) {
			return false
		}
	}
	return true
}

func fortranUsesOnlyIntrinsics(src []byte) bool {
	for _, m := range fortranUse.FindAllSubmatch(src, -1) {
		nature := strings.ToLower(string(m[2]))
		if nature == "intrinsic" {
			continue
		}
		if nature == "non_intrinsic" || !slices.Contains(fortranIntrinsicModules, strings.ToLower(string(m[3]))) {
			return false
		}
	}
	return true
}

func isCloudFormation(head []byte) bool { return cfnTemplate.Match(head) }

// matches reports whether the file belongs to the family.
func (f family) matches(dir, name string) bool {
	if !f.isSource(path.Base(name)) {
		return false
	}
	if f.sniff == nil {
		return true
	}
	file, err := os.Open(filepath.Join(dir, name))
	if err != nil {
		return false
	}
	defer file.Close()
	head, err := io.ReadAll(io.LimitReader(file, sniffBytes))
	return err == nil && f.sniff(head)
}

func kinds() map[string]rule.KindInfo {
	out := map[string]rule.KindInfo{}
	for _, f := range families {
		out[f.kind] = common.LibraryKindInfo()
	}
	out[appimageKind] = rule.KindInfo{
		MatchAttrs:     []string{"desktop"},
		NonEmptyAttrs:  map[string]bool{"binary": true, "desktop": true, "icon": true, "notice": true},
		MergeableAttrs: map[string]bool{"binary": true, "desktop": true, "icon": true, "notice": true},
	}
	out[msixKind] = rule.KindInfo{
		MatchAttrs:     []string{"identity_name"},
		NonEmptyAttrs:  map[string]bool{"binary": true, "identity_name": true, "logo": true, "notice": true, "publisher": true, "version": true},
		MergeableAttrs: map[string]bool{"binary": true, "display_name": true, "identity_name": true, "logo": true, "notice": true, "publisher": true, "version": true},
	}
	out[ociQualityKind] = rule.KindInfo{
		MatchAttrs:     []string{"image"},
		NonEmptyAttrs:  map[string]bool{"image": true},
		MergeableAttrs: map[string]bool{"image": true, "build_files": true},
	}
	return out
}

func generate(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	var result language.GenerateResult
	var claimants []common.Claimant
	unbuildable := map[string]bool{}
	for _, f := range families {
		var srcs []string
		for _, name := range args.RegularFiles {
			if f.matches(args.Dir, name) {
				srcs = append(srcs, name)
			}
		}
		if len(srcs) == 0 || handwritten(args.File, f, srcs) {
			continue
		}
		if f.buildable != nil && !f.buildable(args.Dir, srcs) {
			unbuildable[f.kind] = true
			continue
		}
		sort.Strings(srcs)
		r := rule.NewRule(f.kind, f.target)
		r.SetAttr("srcs", srcs)
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, nil)
		claimants = append(claimants, common.Claimant{Name: f.target, Source: path.Join(args.Rel, srcs[0]), Kind: f.kind})
	}
	if args.File == nil && len(result.Gen) > 0 {
		if owner, ok := common.AncestorOwner(args.Config, args.Rel); ok {
			common.NoticeAncestorOwned(args.Rel, owner)
			return language.GenerateResult{}
		}
	}
	appimages := appimageRules(args)
	result.Gen = append(result.Gen, appimages...)
	for range appimages {
		result.Imports = append(result.Imports, nil)
	}
	if r := msixRule(args); r != nil {
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, nil)
	}
	images := ociImageNames(args.File)
	for _, image := range images {
		r := rule.NewRule(ociQualityKind, image+ociQualitySuffix)
		r.SetAttr("build_files", []string{":" + path.Base(args.File.Path)})
		r.SetAttr("image", ":"+image)
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, nil)
	}
	if err := common.CheckClaims(args.File, args.OtherGen, claimants); err != nil {
		rep.Fail(common.InPackage(args.Rel), "%s: %s: %v", languageName, args.Rel, err)
		return language.GenerateResult{}
	}
	return common.MergeStaleOwned(args.File, result, kinds, func(existing *rule.Rule) bool {
		return !unbuildable[existing.Kind()] && slices.ContainsFunc(families, func(f family) bool {
			return f.kind == existing.Kind() && (f.target == existing.Name() || len(existing.AttrStrings("srcs")) == 0)
		}) || ociStale(existing, images) || appimageStale(existing, args.RegularFiles) || msixStale(existing, args.RegularFiles)
	})
}

// appxManifest is the part of a Package.appxmanifest an msix_package takes.
type appxManifest struct {
	Identity struct {
		Name      string `xml:"Name,attr"`
		Publisher string `xml:"Publisher,attr"`
		Version   string `xml:"Version,attr"`
	} `xml:"Identity"`
	DisplayName string `xml:"Properties>DisplayName"`
	Logo        string `xml:"Properties>Logo"`
}

// msixRule returns an msix_package for a Package.appxmanifest whose identity
// is complete and whose logo is a PNG path inside the package, when a notice
// file sits beside it and the package has exactly one *_binary rule.
func msixRule(args language.GenerateArgs) *rule.Rule {
	if !slices.Contains(args.RegularFiles, msixManifest) {
		return nil
	}
	notices := noticeFiles(args.RegularFiles)
	if len(notices) == 0 {
		return nil
	}
	data, err := os.ReadFile(filepath.Join(args.Dir, msixManifest))
	if err != nil {
		return nil
	}
	var m appxManifest
	if xml.Unmarshal(data, &m) != nil {
		return nil
	}
	logo := path.Clean(strings.ReplaceAll(m.Logo, `\`, "/"))
	if m.Identity.Name == "" || m.Identity.Publisher == "" || m.Identity.Version == "" || !strings.HasSuffix(strings.ToLower(logo), ".png") || path.IsAbs(logo) || logo == ".." || strings.HasPrefix(logo, "../") {
		return nil
	}
	binary := soleBinary(args)
	if binary == "" || handwrittenMsix(args.File, binary, m.Identity.Name) {
		return nil
	}
	r := rule.NewRule(msixKind, binary+msixSuffix)
	r.SetAttr("binary", ":"+binary)
	if m.DisplayName != "" && !strings.HasPrefix(m.DisplayName, "ms-resource:") {
		r.SetAttr("display_name", m.DisplayName)
	}
	r.SetAttr("identity_name", m.Identity.Name)
	r.SetAttr("logo", logo)
	r.SetAttr("notice", notices)
	r.SetAttr("publisher", m.Identity.Publisher)
	r.SetAttr("version", m.Identity.Version)
	return r
}

// handwrittenMsix reports whether the package already has an msix_package
// under another name for the identity.
func handwrittenMsix(file *rule.File, binary, identity string) bool {
	return file != nil && slices.ContainsFunc(file.Rules, func(r *rule.Rule) bool {
		return r.Kind() == msixKind && r.Name() != binary+msixSuffix && r.AttrString("identity_name") == identity
	})
}

// msixStale reports whether existing is a generated msix_package whose
// Package.appxmanifest is gone.
func msixStale(existing *rule.Rule, files []string) bool {
	return existing.Kind() == msixKind && existing.Name() == strings.TrimPrefix(existing.AttrString("binary"), ":")+msixSuffix && !slices.Contains(files, msixManifest)
}

// noticeFiles lists the notice files among files, sorted.
func noticeFiles(files []string) []string {
	var notices []string
	for _, name := range files {
		if slices.Contains(appimageNotices, name) {
			notices = append(notices, name)
		}
	}
	sort.Strings(notices)
	return notices
}

// appimageRules returns an appimage_package for each <name>.desktop file that
// has a <name>.png or <name>.svg icon and a notice file next to it, when the
// package has exactly one *_binary rule.
func appimageRules(args language.GenerateArgs) []*rule.Rule {
	notices := noticeFiles(args.RegularFiles)
	if len(notices) == 0 {
		return nil
	}
	binary := soleBinary(args)
	if binary == "" {
		return nil
	}
	var out []*rule.Rule
	for _, desktop := range args.RegularFiles {
		name, ok := strings.CutSuffix(desktop, ".desktop")
		if !ok || name == "" || strings.Contains(name, "/") {
			continue
		}
		icon := ""
		for _, ext := range []string{".png", ".svg"} {
			if slices.Contains(args.RegularFiles, name+ext) {
				icon = name + ext
				break
			}
		}
		if icon == "" || handwrittenAppimage(args.File, desktop) {
			continue
		}
		r := rule.NewRule(appimageKind, name+appimageSuffix)
		r.SetAttr("app_name", name)
		r.SetAttr("binary", ":"+binary)
		r.SetAttr("desktop", desktop)
		r.SetAttr("icon", icon)
		r.SetAttr("notice", notices)
		r.SetAttr("version", appimageVersion)
		out = append(out, r)
	}
	return out
}

// soleBinary names the one *_binary rule the package declares or another
// language generates for it, or returns "" when there is not exactly one.
func soleBinary(args language.GenerateArgs) string {
	names := map[string]bool{}
	add := func(r *rule.Rule) {
		if strings.HasSuffix(r.Kind(), "_binary") && r.Name() != "" {
			names[r.Name()] = true
		}
	}
	if args.File != nil {
		for _, r := range args.File.Rules {
			add(r)
		}
	}
	for _, r := range args.OtherGen {
		add(r)
	}
	if len(names) != 1 {
		return ""
	}
	for name := range names {
		return name
	}
	return ""
}

// handwrittenAppimage reports whether the package already has an
// appimage_package under another name for the desktop file.
func handwrittenAppimage(file *rule.File, desktop string) bool {
	want := strings.TrimSuffix(desktop, ".desktop") + appimageSuffix
	return file != nil && slices.ContainsFunc(file.Rules, func(r *rule.Rule) bool {
		return r.Kind() == appimageKind && r.Name() != want && r.AttrString("desktop") == desktop
	})
}

// appimageStale reports whether existing is a generated appimage_package whose
// desktop file is gone.
func appimageStale(existing *rule.Rule, files []string) bool {
	desktop := existing.AttrString("desktop")
	return existing.Kind() == appimageKind && existing.Name() == strings.TrimSuffix(desktop, ".desktop")+appimageSuffix && !slices.Contains(files, desktop)
}

// ociImageNames lists the oci_image rules the BUILD file declares.
func ociImageNames(file *rule.File) []string {
	if file == nil {
		return nil
	}
	var names []string
	for _, r := range file.Rules {
		if r.Kind() == ociImageKind && r.Name() != "" {
			names = append(names, r.Name())
		}
	}
	sort.Strings(names)
	return names
}

// ociStale reports whether existing is a quality wrapper for an image the
// package no longer declares.
func ociStale(existing *rule.Rule, images []string) bool {
	if existing.Kind() != ociQualityKind || !strings.HasSuffix(existing.Name(), ociQualitySuffix) {
		return false
	}
	image := strings.TrimSuffix(existing.Name(), ociQualitySuffix)
	return existing.AttrString("image") == ":"+image && !slices.Contains(images, image)
}

// handwritten reports whether the package already has a rule of the kind under
// another name that lists one of the files, which then owns them.
func handwritten(file *rule.File, f family, srcs []string) bool {
	return file != nil && slices.ContainsFunc(file.Rules, func(r *rule.Rule) bool {
		return r.Kind() == f.kind && r.Name() != f.target && slices.ContainsFunc(r.AttrStrings("srcs"), func(s string) bool { return slices.Contains(srcs, s) })
	})
}

func NewLanguage() language.Language {
	kindDefs := map[string]string{}
	var extra, index []string
	for _, f := range families {
		kindDefs[f.kind] = f.defs
		extra = append(extra, f.kind)
		index = append(index, f.kind)
	}
	kindDefs[appimageKind] = "appimage:defs.bzl"
	kindDefs[msixKind] = "msix:defs.bzl"
	kindDefs[ociQualityKind] = "deploy:defs.bzl"
	extra = append(extra, appimageKind, msixKind, ociQualityKind)
	return common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		LibraryKind:    families[0].kind,
		ExtraKinds:     extra[1:],
		Kinds:          kinds(),
		DefsPackage:    families[0].defs,
		KindDefs:       kindDefs,
		IndexKinds:     index[1:],
		ImportAttr:     "srcs",
		IsImportSource: func(string) bool { return true },
		UnresolvedHint: "ownership rules have no dependencies",
		Generate:       generate,
	})
}

func init() { common.RegisterLanguage(NewLanguage) }
