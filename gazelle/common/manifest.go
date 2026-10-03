package common

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path"
	"path/filepath"
	"sort"
	"strings"

	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/merger"
	"github.com/bazelbuild/bazel-gazelle/rule"
	difflib "github.com/pmezard/go-difflib/difflib"
)

const (
	IntendedManifestSchemaMajor = 1
	IntendedManifestSchemaMinor = 0

	EnvIntendedManifest = "DX_GENERATE_INTENDED"
	EnvGenerateScope    = "DX_GENERATE_SCOPE"
	EnvGenerateMode     = "DX_GENERATE_MODE"
)

type ScopeElement struct {
	Element string   `json:"element"`
	Dirs    []string `json:"dirs"`
}

type IntendedEdit struct {
	Start       uint64 `json:"start_byte"`
	End         uint64 `json:"end_byte"`
	Replacement []byte `json:"replacement"`
}

type IntendedFile struct {
	Path            string         `json:"path"`
	ScopeIndex      int            `json:"scope_index"`
	CreateContent   []byte         `json:"create_content,omitempty"`
	OriginalContent []byte         `json:"original_content,omitempty"`
	Edits           []IntendedEdit `json:"edits,omitempty"`
}

type IgnoredImport struct {
	Path     string
	Language string
	Value    string
}

type IntendedIgnoredImport struct {
	Path       string `json:"path"`
	Language   string `json:"language"`
	Import     string `json:"import"`
	ScopeIndex int    `json:"scope_index"`
}

type IntendedScope struct {
	Value           string `json:"value"`
	ResultsComplete bool   `json:"results_complete"`
}

type IntendedManifest struct {
	SchemaMajor    int                     `json:"schema_major"`
	SchemaMinor    int                     `json:"schema_minor"`
	Mode           string                  `json:"mode"`
	Scopes         []IntendedScope         `json:"scopes"`
	Files          []IntendedFile          `json:"files"`
	IgnoredImports []IntendedIgnoredImport `json:"ignored_imports"`
}

type PackageRecord struct {
	Rel      string
	Dir      string
	File     *rule.File
	Gen      []*rule.Rule
	GenKinds []string
	OldKinds []string
	Cfg      *config.Config
}

type ManifestRecorder struct {
	Prefix          string
	IncludeOtherGen bool
	OutPath         string
	Mode            string
	Scopes          []ScopeElement
	ApparentLoads   func(func(string) string) []rule.LoadInfo
	Visited         []PackageRecord
}

func LoadManifestRecorder(prefix string) (*ManifestRecorder, error) {
	outPath := os.Getenv(EnvIntendedManifest)
	if outPath == "" {
		return nil, nil
	}
	rec := &ManifestRecorder{Prefix: prefix, OutPath: outPath}
	mode := os.Getenv(EnvGenerateMode)
	if mode == "" {
		mode = "default"
	}
	if mode != "check" && mode != "default" {
		return nil, rec.Errorf("%s must be \"check\" or \"default\", got %q", EnvGenerateMode, mode)
	}
	rec.Mode = mode
	scopes := []ScopeElement{{Element: "//...", Dirs: []string{""}}}
	if raw := os.Getenv(EnvGenerateScope); raw != "" {
		if err := json.Unmarshal([]byte(raw), &scopes); err != nil {
			return nil, rec.Errorf("malformed %s: %v", EnvGenerateScope, err)
		}
		if len(scopes) == 0 {
			return nil, rec.Errorf("%s must list at least one scope element", EnvGenerateScope)
		}
	}
	rec.Scopes = scopes
	return rec, nil
}

func (r *ManifestRecorder) Errorf(format string, args ...interface{}) error {
	return fmt.Errorf(r.Prefix+": "+format, args...)
}

func UniqueRules(groups ...[]*rule.Rule) []*rule.Rule {
	seen := map[*rule.Rule]bool{}
	var out []*rule.Rule
	for _, group := range groups {
		for _, g := range group {
			if g == nil || seen[g] {
				continue
			}
			seen[g] = true
			out = append(out, g)
		}
	}
	return out
}

func (r *ManifestRecorder) Record(args language.GenerateArgs, res language.GenerateResult) {
	rec := PackageRecord{
		Rel:  args.Rel,
		Dir:  args.Dir,
		File: args.File,
		Cfg:  args.Config,
	}
	if r.IncludeOtherGen {
		rec.Gen = UniqueRules(args.OtherGen, res.Gen)
	} else {
		rec.Gen = UniqueRules(res.Gen)
	}
	for _, g := range rec.Gen {
		rec.GenKinds = append(rec.GenKinds, g.Kind())
	}
	if args.File != nil {
		for _, old := range args.File.Rules {
			rec.OldKinds = append(rec.OldKinds, old.Kind())
		}
	}
	r.Visited = append(r.Visited, rec)
}

func (r *ManifestRecorder) ScopeIndex(rel string) int {
	best := -1
	bestLen := -1
	for i, scope := range r.Scopes {
		for _, dir := range scope.Dirs {
			length := -1
			switch {
			case dir == "":
				length = 0
			case rel == dir:
				length = len(dir)
			case strings.HasPrefix(rel, dir+"/"):
				length = len(dir)
			}
			if length > bestLen {
				bestLen = length
				best = i
			}
		}
	}
	return best
}

func (r *ManifestRecorder) Emit(ignores []IgnoredImport) error {
	manifest := IntendedManifest{
		SchemaMajor:    IntendedManifestSchemaMajor,
		SchemaMinor:    IntendedManifestSchemaMinor,
		Mode:           r.Mode,
		Scopes:         []IntendedScope{},
		Files:          []IntendedFile{},
		IgnoredImports: []IntendedIgnoredImport{},
	}
	for _, scope := range r.Scopes {
		manifest.Scopes = append(manifest.Scopes, IntendedScope{
			Value:           scope.Element,
			ResultsComplete: true,
		})
	}
	for _, rec := range r.Visited {
		index := r.ScopeIndex(rec.Rel)
		if index < 0 {
			return r.Errorf("package %q matches no %s scope element", rec.Rel, EnvGenerateScope)
		}
		file, changed, err := r.Witness(rec)
		if err != nil {
			return err
		}
		if !changed {
			continue
		}
		file.ScopeIndex = index
		manifest.Files = append(manifest.Files, file)
	}
	seenIgnore := map[IntendedIgnoredImport]bool{}
	for _, ignore := range ignores {
		index := r.ScopeIndex(ignore.Path)
		if index < 0 {
			return r.Errorf("ignored import %q matches no %s scope element", ignore.Value, EnvGenerateScope)
		}
		entry := IntendedIgnoredImport{
			Path:       ignore.Path,
			Language:   ignore.Language,
			Import:     ignore.Value,
			ScopeIndex: index,
		}
		if seenIgnore[entry] {
			continue
		}
		seenIgnore[entry] = true
		manifest.IgnoredImports = append(manifest.IgnoredImports, entry)
	}
	sort.SliceStable(manifest.IgnoredImports, func(i, j int) bool {
		a, b := manifest.IgnoredImports[i], manifest.IgnoredImports[j]
		if a.Path != b.Path {
			return a.Path < b.Path
		}
		if a.Language != b.Language {
			return a.Language < b.Language
		}
		return a.Import < b.Import
	})
	data, err := json.Marshal(manifest)
	if err != nil {
		return r.Errorf("cannot encode intended manifest: %v", err) // LCOV_EXCL_LINE - reason: unreachable encode, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
	}
	if err := os.WriteFile(r.OutPath, data, 0o600); err != nil {
		return r.Errorf("cannot write intended manifest: %v", err)
	}
	return nil
}

func (r *ManifestRecorder) Witness(rec PackageRecord) (IntendedFile, bool, error) {
	var file IntendedFile
	if rec.File == nil {
		if len(rec.Gen) == 0 {
			return file, false, nil
		}
		built := rule.EmptyFile(filepath.Join(rec.Dir, rec.Cfg.DefaultBuildFileName()), rec.Rel)
		for _, g := range rec.Gen {
			g.Insert(built)
		}
		loads, err := r.KnownLoads(rec)
		if err != nil {
			return file, false, err
		}
		merger.FixLoads(built, loads)
		file.Path = manifestPath(rec.Rel, rec.Cfg.DefaultBuildFileName())
		file.CreateContent = built.Format()
		return file, true, nil
	}
	original := rec.File.Content
	loads, err := r.KnownLoads(rec)
	if err != nil {
		return file, false, err
	}
	merger.FixLoads(rec.File, loads)
	intended := rec.File.Format()
	if bytes.Equal(original, intended) {
		return file, false, nil
	}
	file.Path = manifestPath(rec.Rel, filepath.Base(rec.File.Path))
	file.OriginalContent = original
	file.Edits = diffLines(original, intended)
	if len(file.Edits) == 0 {
		return file, false, r.Errorf("changed package %q produced no edits", rec.Rel) // LCOV_EXCL_LINE - reason: unreachable encode, issue: 1055, policy: docs/cli/commands/build-test-coverage.md
	}
	return file, true, nil
}

func (r *ManifestRecorder) KnownLoads(rec PackageRecord) ([]rule.LoadInfo, error) {
	var loads []rule.LoadInfo
	if r.ApparentLoads != nil {
		loads = r.ApparentLoads(rec.Cfg.ModuleToApparentName)
	}
	return r.applyKindMappings(rec, loads)
}

func manifestPath(rel, base string) string {
	if rel == "" {
		return base
	}
	return path.Join(rel, base)
}

func (r *ManifestRecorder) applyKindMappings(rec PackageRecord, loads []rule.LoadInfo) ([]rule.LoadInfo, error) {
	var mapped []config.MappedKind
	for _, kind := range append(append([]string{}, rec.GenKinds...), rec.OldKinds...) {
		repl, err := r.replacementKind(rec.Cfg.KindMap, kind)
		if err != nil {
			return nil, err
		}
		if repl != nil {
			mapped = append(mapped, *repl)
		}
	}
	if len(mapped) == 0 {
		return loads, nil
	}
	merged := make([]rule.LoadInfo, len(loads))
	copy(merged, loads)
	for _, repl := range mapped {
		merged = appendOrMergeKindMapping(merged, repl)
	}
	return merged, nil
}

func appendOrMergeKindMapping(loads []rule.LoadInfo, repl config.MappedKind) []rule.LoadInfo {
	for i, load := range loads {
		if load.Name == repl.KindLoad {
			loads[i].Symbols = append(loads[i].Symbols, repl.KindName)
			return loads
		}
	}
	return append(loads, rule.LoadInfo{
		Name:    repl.KindLoad,
		Symbols: []string{repl.KindName},
	})
}

func (r *ManifestRecorder) replacementKind(kindMap map[string]config.MappedKind, kind string) (*config.MappedKind, error) {
	var mapped *config.MappedKind
	seen := make(map[string]struct{})
	for {
		replacement, ok := kindMap[kind]
		if !ok {
			return mapped, nil
		}
		if _, dup := seen[replacement.KindName]; dup {
			return nil, r.Errorf("kind map loop at %q", replacement.KindName)
		}
		seen[replacement.KindName] = struct{}{}
		current := replacement
		mapped = &current
		if kind == replacement.KindName {
			return mapped, nil
		}
		kind = replacement.KindName
	}
}

func nonNilBytes(b []byte) []byte {
	if b == nil {
		return []byte{}
	}
	return b
}

func splitLines(data []byte) [][]byte {
	if len(data) == 0 {
		return nil
	}
	var lines [][]byte
	for len(data) > 0 {
		i := bytes.IndexByte(data, '\n')
		if i < 0 {
			lines = append(lines, data)
			break
		}
		lines = append(lines, data[:i+1])
		data = data[i+1:]
	}
	return lines
}

func diffLines(original, intended []byte) []IntendedEdit {
	oldLines := splitLines(original)
	newLines := splitLines(intended)
	oldOffsets := lineOffsets(oldLines)
	var edits []IntendedEdit
	for _, code := range difflib.NewMatcher(toStrings(oldLines), toStrings(newLines)).GetOpCodes() {
		if code.Tag == 'e' {
			continue
		}
		edit := IntendedEdit{
			Start:       oldOffsets[code.I1],
			End:         oldOffsets[code.I2],
			Replacement: nonNilBytes(bytes.Join(newLines[code.J1:code.J2], nil)),
		}
		if bytes.Equal(original[edit.Start:edit.End], edit.Replacement) {
			continue
		}
		edits = append(edits, edit)
	}
	return edits
}

func lineOffsets(lines [][]byte) []uint64 {
	offsets := make([]uint64, len(lines)+1)
	for i, line := range lines {
		offsets[i+1] = offsets[i] + uint64(len(line))
	}
	return offsets
}

func toStrings(lines [][]byte) []string {
	out := make([]string, len(lines))
	for i, line := range lines {
		out[i] = string(line)
	}
	return out
}
