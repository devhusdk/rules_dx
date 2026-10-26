package main

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func write(t *testing.T, dir string, files map[string]string) []Src {
	t.Helper()
	var srcs []Src
	for name, text := range files {
		path := filepath.Join(dir, name)
		if err := os.WriteFile(path, []byte(text), 0o644); err != nil {
			t.Fatal(err)
		}
		srcs = append(srcs, Src{Exec: path, Rel: "pkg/" + name})
	}
	return srcs
}

func options(srcs []Src) Options {
	return Options{ImportPath: "example.com/pkg", GOOS: "linux", GOARCH: "amd64", Srcs: srcs}
}

func find(t *testing.T, out *Output, recv, name string) Item {
	t.Helper()
	for _, item := range out.Items {
		if item.Recv == recv && item.Name == name {
			return item
		}
	}
	t.Fatalf("no item %s.%s in %+v", recv, name, out.Items)
	return Item{}
}

const lib = `// Package pkg keeps accounts.
//
// # Usage
//
// Call [Open] first.
package pkg

// Limit caps balances.
const Limit = 10

// Kinds of account.
const (
	Checking Kind = iota
	Savings
)

// Kind is an account kind.
type Kind int

// Default is the default store.
var Default Store

// Account holds money.
type Account struct {
	// Owner names the holder.
	Owner string ` + "`json:\"owner\"`" + `
	balance int
}

// NewAccount opens an account.
func NewAccount(owner string) *Account { return &Account{Owner: owner} }

// Deposit adds money.
func (a *Account) Deposit(n int) { a.balance += n }

func (a *Account) audit() {}

// Store loads accounts.
type Store interface {
	// Get returns one account.
	Get(id string) (*Account, error)
}

// Pair is generic.
type Pair[K comparable, V any] struct{ Key K }

// Name is an alias.
type Name = string

// Open opens a store.
func Open() Store { return nil }

func hidden() {}
`

func TestExtractsTheExportedAPI(t *testing.T) {
	dir := t.TempDir()
	out, err := Extract(options(write(t, dir, map[string]string{"lib.go": lib})))
	if err != nil {
		t.Fatal(err)
	}
	if out.Package != "pkg" || out.ImportPath != "example.com/pkg" || out.Schema != Schema {
		t.Fatalf("header: %+v", out)
	}
	if out.Config != "goos=linux;goarch=amd64;tags=;cgo=0;json=1" {
		t.Fatalf("config: %s", out.Config)
	}
	if !strings.Contains(out.Doc, "# Usage") || strings.Contains(out.Doc, "{#") || strings.Contains(out.Doc, "pkg.go.dev") {
		t.Fatalf("package doc: %q", out.Doc)
	}
	if out.File != "pkg/lib.go" || out.Line != 6 {
		t.Fatalf("package source: %s:%d", out.File, out.Line)
	}
	for _, name := range []string{"hidden", "audit", "balance"} {
		for _, item := range out.Items {
			if item.Name == name {
				t.Fatalf("unexported %s leaked", name)
			}
		}
	}
	deposit := find(t, out, "Account", "Deposit")
	if deposit.Kind != "method" || deposit.Signature != "func (a *Account) Deposit(n int)" || deposit.Doc != "Deposit adds money." {
		t.Fatalf("method: %+v", deposit)
	}
	if deposit.File != "pkg/lib.go" || deposit.Line != 34 {
		t.Fatalf("method source: %s:%d", deposit.File, deposit.Line)
	}
	ctor := find(t, out, "", "NewAccount")
	if ctor.Kind != "func" || strings.Contains(ctor.Signature, "return") {
		t.Fatalf("constructor: %+v", ctor)
	}
	account := find(t, out, "", "Account")
	if account.TypeKind != "struct" || !strings.Contains(account.Signature, "unexported fields") || strings.Contains(account.Signature, "balance") {
		t.Fatalf("struct: %+v", account)
	}
	owner := find(t, out, "Account", "Owner")
	if owner.Kind != "field" || owner.Signature != "Owner string `json:\"owner\"`" || owner.Doc != "Owner names the holder." {
		t.Fatalf("field: %+v", owner)
	}
	get := find(t, out, "Store", "Get")
	if get.Kind != "method" || get.Signature != "Get(id string) (*Account, error)" {
		t.Fatalf("interface method: %+v", get)
	}
	if find(t, out, "", "Store").TypeKind != "interface" || find(t, out, "", "Name").TypeKind != "alias" || find(t, out, "", "Kind").TypeKind != "other" {
		t.Fatal("type kinds")
	}
	if pair := find(t, out, "", "Pair"); !strings.Contains(pair.Signature, "Pair[K comparable, V any]") {
		t.Fatalf("generic: %+v", pair)
	}
	savings := find(t, out, "", "Savings")
	if savings.Kind != "const" || !strings.Contains(savings.Signature, "Checking Kind = iota") || savings.Doc != "Kinds of account." {
		t.Fatalf("const group: %+v", savings)
	}
	if find(t, out, "", "Default").Kind != "var" || find(t, out, "", "Limit").Kind != "const" {
		t.Fatal("values")
	}
}

func TestBuildConstraintsAndCgoAreExcluded(t *testing.T) {
	dir := t.TempDir()
	srcs := write(t, dir, map[string]string{
		"lib.go":     "package pkg\n\n// A is here.\nfunc A() {}\n",
		"win.go":     "//go:build windows\n\npackage pkg\n\n// W is windows only.\nfunc W() {}\n",
		"lib_arm.go": "package pkg\n\nfunc Arm() {}\n",
		"c.go":       "package pkg\n\nimport \"C\"\n\nfunc C1() {}\n",
		"x_test.go":  "package pkg\n\nfunc Test() {}\n",
	})
	out, err := Extract(options(srcs))
	if err != nil {
		t.Fatal(err)
	}
	if strings.Join(out.Excluded, ",") != "pkg/c.go,pkg/lib_arm.go,pkg/win.go" {
		t.Fatalf("excluded: %v", out.Excluded)
	}
	if len(out.Items) != 1 || out.Items[0].Name != "A" {
		t.Fatalf("items: %+v", out.Items)
	}
	opts := options(srcs)
	opts.GOOS = "windows"
	opts.Tags = []string{"extra"}
	out, err = Extract(opts)
	if err != nil {
		t.Fatal(err)
	}
	find(t, out, "", "W")
	if out.Config != "goos=windows;goarch=amd64;tags=extra;cgo=0;json=1" {
		t.Fatalf("config: %s", out.Config)
	}
}

func TestFailures(t *testing.T) {
	for _, tc := range []struct {
		name, want string
		files      map[string]string
		mutate     func(*Options)
	}{
		{"parse", "cannot parse", map[string]string{"a.go": "package pkg\nfunc {"}, nil},
		{"mixed", "mixed package names", map[string]string{"a.go": "package a\nfunc A() {}\n", "b.go": "package b\nfunc B() {}\n"}, nil},
		{"main", "package main", map[string]string{"a.go": "package main\nfunc A() {}\n"}, nil},
		{"constrained", "no Go files are left", map[string]string{"a.go": "//go:build ignore\n\npackage a\nfunc A() {}\n"}, nil},
		{"empty", "exports no API", map[string]string{"a.go": "package a\nfunc a() {}\n"}, nil},
		{"importpath", "import path is empty", map[string]string{"a.go": "package a\nfunc A() {}\n"}, func(o *Options) { o.ImportPath = "" }},
		{"duplicate", "listed twice", map[string]string{"a.go": "package a\nfunc A() {}\n"}, func(o *Options) { o.Srcs = append(o.Srcs, o.Srcs[0]) }},
	} {
		t.Run(tc.name, func(t *testing.T) {
			opts := options(write(t, t.TempDir(), tc.files))
			if tc.mutate != nil {
				tc.mutate(&opts)
			}
			_, err := Extract(opts)
			if err == nil || !strings.Contains(err.Error(), tc.want) {
				t.Fatalf("got %v, want %q", err, tc.want)
			}
		})
	}
}

func TestMainWritesJSONOrFails(t *testing.T) {
	dir := t.TempDir()
	srcs := write(t, dir, map[string]string{"lib.go": lib})
	manifest := filepath.Join(dir, "srcs.tsv")
	if err := os.WriteFile(manifest, []byte(srcs[0].Exec+"\tpkg/lib.go\t1\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	out := filepath.Join(dir, "out.json")
	var stderr bytes.Buffer
	if code := Main([]string{"--importpath", "example.com/pkg", "--srcs-file", manifest, "--out", out}, &stderr); code != 0 {
		t.Fatalf("exit %d: %s", code, stderr.String())
	}
	data, err := os.ReadFile(out)
	if err != nil {
		t.Fatal(err)
	}
	var decoded Output
	if err := json.Unmarshal(data, &decoded); err != nil {
		t.Fatal(err)
	}
	if !decoded.Generated || !decoded.Items[0].Generated {
		t.Fatal("the generated flag is lost")
	}
	stderr.Reset()
	missing := filepath.Join(dir, "missing.json")
	if code := Main([]string{"--importpath", "", "--srcs-file", manifest, "--out", missing}, &stderr); code != 1 {
		t.Fatalf("exit %d", code)
	}
	if lines := strings.Count(strings.TrimSpace(stderr.String()), "\n"); lines != 0 || stderr.Len() == 0 {
		t.Fatalf("stderr: %q", stderr.String())
	}
	if _, err := os.Stat(missing); !os.IsNotExist(err) {
		t.Fatal("a failed run wrote output")
	}
	if code := Main([]string{"--srcs-file", manifest}, &stderr); code != 2 {
		t.Fatalf("usage exit %d", code)
	}
}
