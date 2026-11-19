// Package gofumpt formats Go files the way the upstream gofumpt command does.
package gofumpt

import (
	"bytes"
	"errors"
	"flag"
	"fmt"
	"go/parser"
	"go/scanner"
	"go/token"
	"io"
	"os"
	"path/filepath"

	"github.com/rogpeppe/go-internal/diff"
	"golang.org/x/mod/modfile"
	gformat "mvdan.cc/gofumpt/format"
)

// Main formats the Go files named in args and returns the exit code.
// Like upstream, -d exits 1 when a file differs and any error exits 2.
func Main(args []string, stdout, stderr io.Writer) int {
	fs := flag.NewFlagSet("gofumpt", flag.ContinueOnError)
	fs.SetOutput(stderr)
	list := fs.Bool("l", false, "list files whose formatting differs from gofumpt's")
	write := fs.Bool("w", false, "write result to the source file instead of stdout")
	doDiff := fs.Bool("d", false, "display diffs instead of rewriting files")
	lang := fs.String("lang", "", "target Go version such as go1.26 (default from go.mod)")
	modpath := fs.String("modpath", "", "Go module path of the files (default from go.mod)")
	var extra gformat.Extra
	fs.Var(&extra, "extra", "enable extra rules, such as -extra=group_params")
	if err := fs.Parse(args); err != nil {
		return 2
	}
	if fs.NArg() == 0 {
		fmt.Fprintln(stderr, "gofumpt: usage: gofumpt [-l] [-w] [-d] [-lang go1.X] [-modpath PATH] [-extra RULES] file.go ...")
		return 2
	}
	mode := mode{list: *list, write: *write, diff: *doDiff}
	opts := gformat.Options{LangVersion: *lang, ModulePath: *modpath, Extra: extra}
	code := 0
	for _, arg := range fs.Args() {
		err := formatFile(filepath.Clean(arg), opts, mode, stdout)
		switch {
		case errors.Is(err, errDiffers):
			code = max(code, 1)
		case err != nil:
			scanner.PrintError(stderr, err)
			code = 2
		}
	}
	return code
}

type mode struct{ list, write, diff bool }

var errDiffers = errors.New("formatting differs from gofumpt's")

func formatFile(path string, opts gformat.Options, m mode, stdout io.Writer) error {
	info, err := os.Stat(path)
	if err != nil {
		return err
	}
	if info.IsDir() {
		return fmt.Errorf("%s: is a directory; name the Go files", path)
	}
	src, err := os.ReadFile(path)
	if err != nil {
		return err
	}
	// Parse with the file name first so syntax errors name the file, as upstream does.
	if _, err = parser.ParseFile(token.NewFileSet(), path, src, parser.ParseComments|parser.SkipObjectResolution); err != nil {
		return err
	}
	if opts.LangVersion == "" || opts.ModulePath == "" {
		if err = fromGoMod(path, &opts); err != nil {
			return err
		}
	}
	res, err := gformat.Source(src, opts)
	if err != nil {
		return err
	}
	if bytes.Equal(src, res) {
		if !m.list && !m.write && !m.diff {
			_, err = stdout.Write(res)
		}
		return err
	}
	if m.list {
		if _, err = fmt.Fprintln(stdout, path); err != nil {
			return err
		}
	}
	if m.write {
		if err = os.WriteFile(path, res, info.Mode().Perm()); err != nil {
			return err
		}
	}
	if m.diff {
		name := filepath.ToSlash(path)
		if _, err = stdout.Write(diff.Diff(name+".orig", src, name, res)); err != nil {
			return err
		}
		return errDiffers
	}
	if !m.list && !m.write {
		_, err = stdout.Write(res)
	}
	return err
}

// fromGoMod fills an unset -lang or -modpath from the nearest go.mod above path.
// A missing or invalid go.mod leaves them unset, as upstream does.
func fromGoMod(path string, opts *gformat.Options) error {
	abs, err := filepath.Abs(path)
	if err != nil {
		return err
	}
	for dir := filepath.Dir(abs); ; {
		gomod := filepath.Join(dir, "go.mod")
		if data, readErr := os.ReadFile(gomod); readErr == nil {
			file, parseErr := modfile.Parse(gomod, data, nil)
			if parseErr != nil {
				return nil
			}
			if opts.LangVersion == "" {
				// A go.mod without a go directive means go 1.16.
				opts.LangVersion = "go1.16"
				if file.Go != nil {
					opts.LangVersion = "go" + file.Go.Version
				}
			}
			if opts.ModulePath == "" && file.Module != nil {
				opts.ModulePath = file.Module.Mod.Path
			}
			return nil
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			return nil
		}
		dir = parent
	}
}
