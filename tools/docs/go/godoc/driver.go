// Command godoc_json extracts one Go package's exported API with go/doc and writes it as JSON.
package main

import (
	"bufio"
	"bytes"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"go/ast"
	"go/build"
	"go/doc"
	"go/doc/comment"
	"go/format"
	"go/parser"
	"go/token"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
)

// Schema is the version of the JSON document the driver writes.
const Schema = 1

// Options are the inputs of one extraction.
type Options struct {
	ImportPath string
	GOOS       string
	GOARCH     string
	Tags       []string
	Srcs       []Src
}

// Src is one source file of the package.
type Src struct {
	Exec      string
	Rel       string
	Generated bool
}

// Output is the JSON document the driver writes.
type Output struct {
	Schema     int      `json:"schema"`
	ImportPath string   `json:"importpath"`
	Package    string   `json:"package"`
	Config     string   `json:"config"`
	Excluded   []string `json:"excluded"`
	Doc        string   `json:"doc"`
	File       string   `json:"file"`
	Line       int      `json:"line"`
	Generated  bool     `json:"generated"`
	Items      []Item   `json:"items"`
}

// Item is one exported declaration.
type Item struct {
	Kind      string `json:"kind"`
	Name      string `json:"name"`
	Recv      string `json:"recv"`
	TypeKind  string `json:"type_kind"`
	Signature string `json:"signature"`
	Doc       string `json:"doc"`
	File      string `json:"file"`
	Line      int    `json:"line"`
	Generated bool   `json:"generated"`
}

// Config returns the configuration identity of one extraction.
func Config(opts Options) string {
	return "goos=" + opts.GOOS + ";goarch=" + opts.GOARCH + ";tags=" + strings.Join(opts.Tags, ",") + ";cgo=0;json=" + strconv.Itoa(Schema)
}

// Main runs the driver on argv and returns the process exit code.
func Main(args []string, stderr io.Writer) int {
	flags := flag.NewFlagSet("godoc_json", flag.ContinueOnError)
	flags.SetOutput(stderr)
	importPath := flags.String("importpath", "", "import path of the package")
	goos := flags.String("goos", "linux", "GOOS the build constraints are evaluated for")
	goarch := flags.String("goarch", "amd64", "GOARCH the build constraints are evaluated for")
	tags := flags.String("tags", "", "comma-separated build tags")
	srcsFile := flags.String("srcs-file", "", "tab-separated manifest of exec path, repo path and generated flag")
	out := flags.String("out", "", "JSON file to write")
	if err := flags.Parse(args); err != nil {
		return 2
	}
	if flags.NArg() != 0 || *srcsFile == "" || *out == "" {
		fmt.Fprintln(stderr, "godoc_json: usage: godoc_json --importpath PATH --srcs-file FILE --out FILE [--goos OS] [--goarch ARCH] [--tags A,B]")
		return 2
	}
	srcs, err := ReadManifest(*srcsFile)
	if err != nil {
		fmt.Fprintln(stderr, "godoc_json: "+err.Error())
		return 1
	}
	opts := Options{ImportPath: *importPath, GOOS: *goos, GOARCH: *goarch, Tags: splitTags(*tags), Srcs: srcs}
	result, err := Extract(opts)
	if err != nil {
		fmt.Fprintln(stderr, "godoc_json: "+*importPath+": "+err.Error())
		return 1
	}
	var buf bytes.Buffer
	encoder := json.NewEncoder(&buf)
	encoder.SetEscapeHTML(false)
	encoder.SetIndent("", "  ")
	if err := encoder.Encode(result); err != nil {
		fmt.Fprintln(stderr, "godoc_json: "+err.Error())
		return 1
	}
	if err := os.WriteFile(*out, buf.Bytes(), 0o644); err != nil {
		fmt.Fprintln(stderr, "godoc_json: "+err.Error())
		return 1
	}
	return 0
}

func splitTags(tags string) []string {
	var result []string
	for _, tag := range strings.Split(tags, ",") {
		if tag = strings.TrimSpace(tag); tag != "" {
			result = append(result, tag)
		}
	}
	sort.Strings(result)
	return result
}

// ReadManifest reads the tab-separated srcs manifest.
func ReadManifest(path string) ([]Src, error) {
	file, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer file.Close()
	var srcs []Src
	scanner := bufio.NewScanner(file)
	for scanner.Scan() {
		line := strings.TrimRight(scanner.Text(), "\r")
		if line == "" {
			continue
		}
		fields := strings.Split(line, "\t")
		if len(fields) != 3 || fields[0] == "" || fields[1] == "" || (fields[2] != "0" && fields[2] != "1") {
			return nil, fmt.Errorf("malformed srcs manifest line %q", line)
		}
		srcs = append(srcs, Src{Exec: fields[0], Rel: fields[1], Generated: fields[2] == "1"})
	}
	return srcs, scanner.Err()
}

type parsed struct {
	src  Src
	file *ast.File
}

// Extract reads, filters and documents one package.
func Extract(opts Options) (*Output, error) {
	if opts.ImportPath == "" {
		return nil, errors.New("the import path is empty")
	}
	ctxt := build.Context{
		GOOS:        opts.GOOS,
		GOARCH:      opts.GOARCH,
		Compiler:    "gc",
		BuildTags:   opts.Tags,
		ReleaseTags: append([]string(nil), build.Default.ReleaseTags...),
		CgoEnabled:  false,
	}
	srcs := append([]Src(nil), opts.Srcs...)
	sort.Slice(srcs, func(i, j int) bool { return srcs[i].Rel < srcs[j].Rel })
	seen := map[string]bool{}
	fset := token.NewFileSet()
	var files []parsed
	excluded := []string{}
	generated := map[string]bool{}
	for _, src := range srcs {
		if seen[src.Rel] {
			return nil, fmt.Errorf("%s is listed twice", src.Rel)
		}
		seen[src.Rel] = true
		if !strings.HasSuffix(src.Rel, ".go") || strings.HasSuffix(src.Rel, "_test.go") {
			continue
		}
		generated[src.Rel] = src.Generated
		match, err := ctxt.MatchFile(filepath.Dir(src.Exec), filepath.Base(src.Exec))
		if err != nil {
			return nil, fmt.Errorf("%s: %v", src.Rel, err)
		}
		if !match {
			excluded = append(excluded, src.Rel)
			continue
		}
		data, err := os.ReadFile(src.Exec)
		if err != nil {
			return nil, fmt.Errorf("%s: %v", src.Rel, err)
		}
		file, err := parser.ParseFile(fset, src.Rel, data, parser.ParseComments|parser.SkipObjectResolution)
		if err != nil {
			return nil, fmt.Errorf("cannot parse %v", err)
		}
		if importsC(file) {
			excluded = append(excluded, src.Rel)
			continue
		}
		files = append(files, parsed{src: src, file: file})
	}
	if len(files) == 0 {
		return nil, errors.New("no Go files are left after build constraints for " + Config(opts))
	}
	name := files[0].file.Name.Name
	for _, f := range files[1:] {
		if f.file.Name.Name != name {
			return nil, fmt.Errorf("mixed package names %s and %s in %s", name, f.file.Name.Name, f.src.Rel)
		}
	}
	if name == "main" {
		return nil, errors.New("package main has no importable API")
	}
	asts := make([]*ast.File, len(files))
	for i, f := range files {
		asts[i] = f.file
	}
	pkg, err := doc.NewFromFiles(fset, asts, opts.ImportPath)
	if err != nil {
		return nil, err
	}
	x := &extractor{fset: fset, pkg: pkg, generated: generated}
	out := &Output{
		Schema:     Schema,
		ImportPath: opts.ImportPath,
		Package:    pkg.Name,
		Config:     Config(opts),
		Excluded:   excluded,
		Doc:        x.markdown(pkg.Doc),
	}
	home := files[0]
	for _, f := range files {
		if f.file.Doc != nil {
			home = f
			break
		}
	}
	out.File, out.Line, out.Generated = x.where(home.file.Package)
	x.values(pkg.Consts, "const")
	x.values(pkg.Vars, "var")
	x.funcs(pkg.Funcs, "")
	for _, t := range pkg.Types {
		x.values(t.Consts, "const")
		x.values(t.Vars, "var")
		x.funcs(t.Funcs, "")
		x.typ(t)
		x.funcs(t.Methods, t.Name)
	}
	if len(x.items) == 0 {
		return nil, errors.New("the package exports no API")
	}
	sort.Slice(x.items, func(i, j int) bool {
		a, b := x.items[i], x.items[j]
		if a.Recv != b.Recv {
			return a.Recv < b.Recv
		}
		return a.Name < b.Name
	})
	out.Items = x.items
	return out, nil
}

func importsC(file *ast.File) bool {
	for _, spec := range file.Imports {
		if path, err := strconv.Unquote(spec.Path.Value); err == nil && path == "C" {
			return true
		}
	}
	return false
}

type extractor struct {
	fset      *token.FileSet
	pkg       *doc.Package
	generated map[string]bool
	items     []Item
}

func (x *extractor) where(pos token.Pos) (string, int, bool) {
	position := x.fset.Position(pos)
	return filepath.ToSlash(position.Filename), position.Line, x.generated[position.Filename]
}

func (x *extractor) markdown(text string) string {
	if strings.TrimSpace(text) == "" {
		return ""
	}
	printer := &comment.Printer{
		HeadingLevel: 1,
		HeadingID:    func(*comment.Heading) string { return "" },
		DocLinkURL:   func(*comment.DocLink) string { return "" },
	}
	return strings.TrimRight(string(printer.Markdown(x.pkg.Parser().Parse(text))), "\n")
}

func (x *extractor) source(node any) string {
	var buf bytes.Buffer
	if err := format.Node(&buf, x.fset, node); err != nil {
		return ""
	}
	return buf.String()
}

func (x *extractor) add(item Item, pos token.Pos) {
	item.File, item.Line, item.Generated = x.where(pos)
	x.items = append(x.items, item)
}

func (x *extractor) values(values []*doc.Value, kind string) {
	for _, value := range values {
		decl := *value.Decl
		decl.Doc = nil
		signature := x.source(&decl)
		for _, spec := range value.Decl.Specs {
			valueSpec, ok := spec.(*ast.ValueSpec)
			if !ok {
				continue
			}
			text := value.Doc
			if valueSpec.Doc != nil {
				text = valueSpec.Doc.Text()
			} else if valueSpec.Comment != nil {
				text = valueSpec.Comment.Text()
			}
			for _, ident := range valueSpec.Names {
				if !token.IsExported(ident.Name) {
					continue
				}
				x.add(Item{Kind: kind, Name: ident.Name, Signature: signature, Doc: x.markdown(text)}, ident.Pos())
			}
		}
	}
}

func (x *extractor) funcs(funcs []*doc.Func, recv string) {
	for _, fn := range funcs {
		if fn.Level > 0 || fn.Decl == nil {
			continue
		}
		decl := *fn.Decl
		decl.Body = nil
		decl.Doc = nil
		kind := "func"
		if recv != "" {
			kind = "method"
		}
		x.add(Item{Kind: kind, Name: fn.Name, Recv: recv, Signature: x.source(&decl), Doc: x.markdown(fn.Doc)}, fn.Decl.Pos())
	}
}

func (x *extractor) typ(t *doc.Type) {
	var spec *ast.TypeSpec
	for _, s := range t.Decl.Specs {
		if typeSpec, ok := s.(*ast.TypeSpec); ok && typeSpec.Name.Name == t.Name {
			spec = typeSpec
		}
	}
	if spec == nil {
		return
	}
	decl := &ast.GenDecl{Tok: token.TYPE, TokPos: t.Decl.TokPos, Specs: []ast.Spec{spec}}
	typeKind := "other"
	switch kind := spec.Type.(type) {
	case *ast.StructType:
		typeKind = "struct"
		if !spec.Assign.IsValid() {
			x.fields(t.Name, kind.Fields)
		}
	case *ast.InterfaceType:
		typeKind = "interface"
		if !spec.Assign.IsValid() {
			x.interfaceMethods(t.Name, kind.Methods)
		}
	}
	if spec.Assign.IsValid() {
		typeKind = "alias"
	}
	text := t.Doc
	if text == "" && spec.Doc != nil {
		text = spec.Doc.Text()
	}
	x.add(Item{Kind: "type", Name: t.Name, TypeKind: typeKind, Signature: x.source(decl), Doc: x.markdown(text)}, spec.Name.Pos())
}

func fieldDoc(field *ast.Field) string {
	if field.Doc != nil {
		return field.Doc.Text()
	}
	if field.Comment != nil {
		return field.Comment.Text()
	}
	return ""
}

func embeddedName(expr ast.Expr) *ast.Ident {
	switch e := expr.(type) {
	case *ast.Ident:
		return e
	case *ast.StarExpr:
		return embeddedName(e.X)
	case *ast.SelectorExpr:
		return e.Sel
	case *ast.IndexExpr:
		return embeddedName(e.X)
	case *ast.IndexListExpr:
		return embeddedName(e.X)
	}
	return nil
}

func (x *extractor) fields(recv string, list *ast.FieldList) {
	if list == nil {
		return
	}
	for _, field := range list.List {
		names := field.Names
		if len(names) == 0 {
			if ident := embeddedName(field.Type); ident != nil {
				names = []*ast.Ident{ident}
			}
		}
		for _, ident := range names {
			if !token.IsExported(ident.Name) {
				continue
			}
			signature := x.source(field.Type)
			if len(field.Names) > 0 {
				signature = ident.Name + " " + signature
			}
			if field.Tag != nil {
				signature += " " + field.Tag.Value
			}
			x.add(Item{Kind: "field", Name: ident.Name, Recv: recv, Signature: signature, Doc: x.markdown(fieldDoc(field))}, ident.Pos())
		}
	}
}

func (x *extractor) interfaceMethods(recv string, list *ast.FieldList) {
	if list == nil {
		return
	}
	for _, field := range list.List {
		funcType, ok := field.Type.(*ast.FuncType)
		if !ok {
			continue
		}
		for _, ident := range field.Names {
			if !token.IsExported(ident.Name) {
				continue
			}
			signature := ident.Name + strings.TrimPrefix(x.source(funcType), "func")
			x.add(Item{Kind: "method", Name: ident.Name, Recv: recv, Signature: signature, Doc: x.markdown(fieldDoc(field))}, ident.Pos())
		}
	}
}
