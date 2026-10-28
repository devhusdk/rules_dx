package gotype

import (
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"go/ast"
	"go/build"
	"go/importer"
	"go/parser"
	"go/scanner"
	"go/token"
	"go/types"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

type config struct {
	ImportPath  string
	GoFiles     []string
	OwnedFiles  []string
	ImportMap   map[string]string
	PackageFile map[string]string
	Standard    map[string]bool
	GoVersion   string
	GOOS        string
	GOARCH      string
	Tags        []string
	Cgo         bool
	StdlibDir   string
}

type diagnostic struct {
	Posn    string `json:"posn"`
	Message string `json:"message"`
}

type failure struct {
	Error string `json:"error"`
}

type result struct {
	diagnostics map[string][]diagnostic
	failure     string
}

func (r result) MarshalJSON() ([]byte, error) {
	if r.failure != "" {
		return json.Marshal(map[string]any{"typecheck": failure{Error: r.failure}})
	}
	out := map[string]any{}
	for rule, list := range r.diagnostics {
		out[rule] = list
	}
	return json.Marshal(out)
}

type resolver struct {
	cfg      *config
	inner    types.ImporterFrom
	override map[string]*types.Package
	missing  []string
}

func (r *resolver) Import(path string) (*types.Package, error) {
	return r.ImportFrom(path, "", 0)
}

func (r *resolver) ImportFrom(path, _ string, _ types.ImportMode) (*types.Package, error) {
	if path == "unsafe" {
		return types.Unsafe, nil
	}
	if mapped, ok := r.cfg.ImportMap[path]; ok {
		path = mapped
	}
	if pkg, ok := r.override[path]; ok {
		return pkg, nil
	}
	pkg, err := r.inner.ImportFrom(path, "", 0)
	if err != nil {
		r.missing = append(r.missing, err.Error())
	}
	return pkg, err
}

func packageFile(cfg *config, path string) (string, bool) {
	if file, ok := cfg.PackageFile[path]; ok {
		return file, true
	}
	if cfg.StdlibDir == "" {
		return "", false
	}
	file := filepath.Join(cfg.StdlibDir, filepath.FromSlash(path)+".a")
	if _, err := os.Stat(file); err != nil {
		return "", false
	}
	return file, true
}

func newResolver(fset *token.FileSet, cfg *config) *resolver {
	lookup := func(path string) (io.ReadCloser, error) {
		file, ok := packageFile(cfg, path)
		if !ok {
			return nil, fmt.Errorf("no export data for %q", path)
		}
		return os.Open(file)
	}
	inner := importer.ForCompiler(fset, "gc", lookup).(types.ImporterFrom)
	return &resolver{cfg: cfg, inner: inner, override: map[string]*types.Package{}}
}

func matchedFiles(cfg *config) ([]string, error) {
	ctxt := build.Default
	ctxt.GOOS = cfg.GOOS
	ctxt.GOARCH = cfg.GOARCH
	ctxt.BuildTags = cfg.Tags
	ctxt.CgoEnabled = cfg.Cgo
	ctxt.UseAllFiles = false
	var files []string
	for _, path := range cfg.GoFiles {
		ok, err := ctxt.MatchFile(filepath.Dir(path), filepath.Base(path))
		if err != nil {
			return nil, err
		}
		if ok {
			files = append(files, path)
		}
	}
	return files, nil
}

func position(fset *token.FileSet, pos token.Pos) string {
	p := fset.Position(pos)
	return fmt.Sprintf("%s:%d:%d", p.Filename, p.Line, p.Column)
}

func syntaxDiagnostics(err error) []diagnostic {
	var list scanner.ErrorList
	if !errors.As(err, &list) {
		return []diagnostic{{Posn: "-:1:1", Message: err.Error()}}
	}
	out := make([]diagnostic, 0, len(list))
	for _, e := range list {
		out = append(out, diagnostic{
			Posn:    fmt.Sprintf("%s:%d:%d", e.Pos.Filename, e.Pos.Line, max(e.Pos.Column, 1)),
			Message: e.Msg,
		})
	}
	return out
}

func check(cfg *config) result {
	if cfg.GoVersion != "" && !strings.HasPrefix(cfg.GoVersion, "go") {
		return result{failure: fmt.Sprintf("invalid GoVersion %q", cfg.GoVersion)}
	}
	files, err := matchedFiles(cfg)
	if err != nil {
		return result{failure: err.Error()}
	}
	fset := token.NewFileSet()
	var syntax []diagnostic
	var internal, external []*ast.File
	for _, path := range files {
		file, err := parser.ParseFile(fset, path, nil, parser.SkipObjectResolution|parser.AllErrors)
		if err != nil {
			syntax = append(syntax, syntaxDiagnostics(err)...)
		}
		if file == nil {
			continue
		}
		if strings.HasSuffix(file.Name.Name, "_test") && strings.HasSuffix(path, "_test.go") {
			external = append(external, file)
		} else {
			internal = append(internal, file)
		}
	}
	if len(syntax) > 0 {
		return result{diagnostics: map[string][]diagnostic{"syntax": syntax}}
	}
	owned := map[string]bool{}
	for _, path := range cfg.OwnedFiles {
		owned[path] = true
	}
	var typed []diagnostic
	var foreign []string
	collect := func(err error) {
		terr, ok := err.(types.Error)
		if !ok {
			typed = append(typed, diagnostic{Posn: "-:1:1", Message: err.Error()})
			return
		}
		name := terr.Fset.Position(terr.Pos).Filename
		if len(owned) > 0 && !owned[name] {
			foreign = append(foreign, name)
			return
		}
		typed = append(typed, diagnostic{Posn: position(terr.Fset, terr.Pos), Message: terr.Msg})
	}
	res := newResolver(fset, cfg)
	conf := types.Config{
		Error:       collect,
		FakeImportC: true,
		GoVersion:   cfg.GoVersion,
		Importer:    res,
		Sizes:       types.SizesFor("gc", cfg.GOARCH),
	}
	var pkg *types.Package
	if len(internal) > 0 || len(external) == 0 {
		pkg, _ = conf.Check(cfg.ImportPath, fset, internal, nil)
		if pkg != nil {
			res.override[cfg.ImportPath] = pkg
		}
	}
	if len(external) > 0 {
		_, _ = conf.Check(cfg.ImportPath+"_test", fset, external, nil)
	}
	if len(res.missing) > 0 {
		return result{failure: res.missing[0]}
	}
	if len(foreign) > 0 {
		sort.Strings(foreign)
		return result{failure: "diagnostic in file the target does not own: " + foreign[0]}
	}
	if len(typed) == 0 {
		return result{diagnostics: map[string][]diagnostic{}}
	}
	return result{diagnostics: map[string][]diagnostic{"typecheck": typed}}
}

func run(configPath, outPath string) error {
	raw, err := os.ReadFile(configPath)
	if err != nil {
		return err
	}
	var cfg config
	if err := json.Unmarshal(raw, &cfg); err != nil {
		return fmt.Errorf("parse config: %w", err)
	}
	if cfg.ImportPath == "" {
		return errors.New("config has no ImportPath")
	}
	res := check(&cfg)
	encoded, err := json.Marshal(map[string]result{cfg.ImportPath: res})
	if err != nil {
		return err
	}
	if outPath == "" {
		_, err = os.Stdout.Write(append(encoded, '\n'))
		return err
	}
	return os.WriteFile(outPath, append(encoded, '\n'), 0o644)
}

func Main(args []string, stderr io.Writer) int {
	flags := flag.NewFlagSet("gotype", flag.ContinueOnError)
	flags.SetOutput(stderr)
	out := flags.String("out", "", "write the JSON report to this file")
	if err := flags.Parse(args); err != nil {
		return 2
	}
	if flags.NArg() != 1 {
		fmt.Fprintln(stderr, "usage: gotype [-out FILE] CONFIG")
		return 2
	}
	if err := run(flags.Arg(0), *out); err != nil {
		fmt.Fprintln(stderr, "gotype:", err)
		return 1
	}
	return 0
}
