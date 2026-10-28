package unit

import (
	"bytes"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"go/build"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"sort"
	"strings"

	"golang.org/x/tools/go/analysis"
	"golang.org/x/tools/go/analysis/unitchecker"
)

const ChildEnv = "DX_UNIT_CHILD"

type Config struct {
	ID          string
	ImportPath  string
	GoFiles     []string
	OwnedFiles  []string
	ImportMap   map[string]string
	PackageFile map[string]string
	Standard    map[string]bool
	PackageVetx map[string]string
	GoVersion   string
	GOOS        string
	GOARCH      string
	Tags        []string
	Cgo         bool
	VetxOnly    bool
	VetxOutput  string
}

type checkerConfig struct {
	ID          string
	Compiler    string
	ImportPath  string
	GoVersion   string
	GoFiles     []string
	ImportMap   map[string]string
	PackageFile map[string]string
	Standard    map[string]bool
	PackageVetx map[string]string
	VetxOnly    bool
	VetxOutput  string
	Stdout      string
}

type childPayload struct {
	Extra  []string
	GOOS   string
	GOARCH string
}

type diagnostic struct {
	Posn    string `json:"posn"`
	End     string `json:"end,omitempty"`
	Message string `json:"message"`
}

type failure struct {
	Error string `json:"error"`
}

func MatchedFiles(cfg *Config) ([]string, error) {
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

func fileOf(posn string) string {
	for range 2 {
		i := strings.LastIndex(posn, ":")
		if i < 0 {
			return posn
		}
		posn = posn[:i]
	}
	return posn
}

func filterOwned(tree map[string]map[string]json.RawMessage, owned []string) map[string]map[string]json.RawMessage {
	if len(owned) == 0 {
		return tree
	}
	set := map[string]bool{}
	for _, path := range owned {
		set[path] = true
	}
	for id, byAnalyzer := range tree {
		for name, raw := range byAnalyzer {
			var list []diagnostic
			if err := json.Unmarshal(raw, &list); err != nil {
				continue
			}
			kept := []diagnostic{}
			for _, d := range list {
				if set[fileOf(d.Posn)] {
					kept = append(kept, d)
				}
			}
			if len(kept) == 0 {
				delete(byAnalyzer, name)
				continue
			}
			encoded, err := json.Marshal(kept)
			if err != nil {
				continue
			}
			byAnalyzer[name] = encoded
		}
		tree[id] = byAnalyzer
	}
	return tree
}

func emit(out string, stdout io.Writer, cfg *Config, tree map[string]map[string]json.RawMessage) error {
	if tree == nil {
		tree = map[string]map[string]json.RawMessage{}
	}
	if _, ok := tree[cfg.ImportPath]; !ok {
		tree[cfg.ImportPath] = map[string]json.RawMessage{}
	}
	encoded, err := json.Marshal(tree)
	if err != nil {
		return err
	}
	encoded = append(encoded, '\n')
	if out == "" {
		_, err = stdout.Write(encoded)
		return err
	}
	return os.WriteFile(out, encoded, 0o644)
}

func emitError(out string, stdout io.Writer, cfg *Config, tool, msg string) error {
	raw, err := json.Marshal(failure{Error: strings.TrimSpace(msg)})
	if err != nil {
		return err
	}
	return emit(out, stdout, cfg, map[string]map[string]json.RawMessage{cfg.ImportPath: {tool: raw}})
}

func touch(path string) error {
	if path == "" {
		return nil
	}
	if _, err := os.Stat(path); err == nil {
		return nil
	}
	return os.WriteFile(path, nil, 0o644)
}

func runChild(analyzers []*analysis.Analyzer, configure func([]string) error) {
	var payload childPayload
	if err := json.Unmarshal([]byte(os.Getenv(ChildEnv)), &payload); err != nil {
		fmt.Fprintln(os.Stderr, "bad child payload:", err)
		os.Exit(2)
	}
	build.Default.GOOS = payload.GOOS
	build.Default.GOARCH = payload.GOARCH
	if configure != nil {
		if err := configure(payload.Extra); err != nil {
			fmt.Fprintln(os.Stderr, err)
			os.Exit(2)
		}
	}
	unitchecker.Main(analyzers...)
}

func run(tool string, analyzers []*analysis.Analyzer, configure func([]string) error, args []string, stdout, stderr io.Writer) (int, error) {
	flags := flag.NewFlagSet(tool, flag.ContinueOnError)
	flags.SetOutput(stderr)
	out := flags.String("out", "", "write the JSON report to this file")
	if err := flags.Parse(args); err != nil {
		return 2, nil
	}
	if flags.NArg() < 1 {
		return 2, errors.New("usage: " + tool + " [-out FILE] CONFIG [DRIVER FLAGS]")
	}
	extra := flags.Args()[1:]
	raw, err := os.ReadFile(flags.Arg(0))
	if err != nil {
		return 1, err
	}
	var cfg Config
	if err := json.Unmarshal(raw, &cfg); err != nil {
		return 1, fmt.Errorf("parse config: %w", err)
	}
	if cfg.ImportPath == "" {
		return 1, errors.New("config has no ImportPath")
	}
	if configure != nil {
		if err := configure(extra); err != nil {
			return 1, err
		}
	}
	files, err := MatchedFiles(&cfg)
	if err != nil {
		return 1, emitError(*out, stdout, &cfg, tool, err.Error())
	}
	if err := touch(cfg.VetxOutput); err != nil {
		return 1, err
	}
	if len(files) == 0 {
		return 0, emit(*out, stdout, &cfg, nil)
	}
	dir, err := os.MkdirTemp("", "dx-unit-*")
	if err != nil {
		return 1, err
	}
	defer os.RemoveAll(dir)
	report := filepath.Join(dir, "report.json")
	id := cfg.ID
	if id == "" {
		id = cfg.ImportPath
	}
	inner := checkerConfig{
		ID: id, Compiler: "gc", ImportPath: cfg.ImportPath, GoVersion: cfg.GoVersion,
		GoFiles: files, ImportMap: cfg.ImportMap, PackageFile: cfg.PackageFile,
		Standard: cfg.Standard, PackageVetx: cfg.PackageVetx, VetxOnly: cfg.VetxOnly,
		VetxOutput: cfg.VetxOutput, Stdout: report,
	}
	encoded, err := json.Marshal(inner)
	if err != nil {
		return 1, err
	}
	cfgPath := filepath.Join(dir, "unit.cfg")
	if err := os.WriteFile(cfgPath, encoded, 0o644); err != nil {
		return 1, err
	}
	payload, err := json.Marshal(childPayload{Extra: extra, GOOS: cfg.GOOS, GOARCH: cfg.GOARCH})
	if err != nil {
		return 1, err
	}
	exe, err := os.Executable()
	if err != nil {
		return 1, err
	}
	var errBuf bytes.Buffer
	cmd := exec.Command(exe, "-json", cfgPath)
	cmd.Env = append(os.Environ(), ChildEnv+"="+string(payload))
	cmd.Stderr = &errBuf
	if err := cmd.Run(); err != nil {
		msg := errBuf.String()
		if strings.TrimSpace(msg) == "" {
			msg = err.Error()
		}
		return 0, emitError(*out, stdout, &cfg, tool, msg)
	}
	data, err := os.ReadFile(report)
	if err != nil || len(bytes.TrimSpace(data)) == 0 {
		if cfg.VetxOnly {
			return 0, emit(*out, stdout, &cfg, nil)
		}
		return 0, emitError(*out, stdout, &cfg, tool, "analysis produced no report: "+errBuf.String())
	}
	var tree map[string]map[string]json.RawMessage
	if err := json.Unmarshal(data, &tree); err != nil {
		return 0, emitError(*out, stdout, &cfg, tool, "unreadable report: "+err.Error())
	}
	renamed := map[string]map[string]json.RawMessage{}
	for _, byAnalyzer := range tree {
		renamed[cfg.ImportPath] = mergeTools(renamed[cfg.ImportPath], byAnalyzer)
	}
	return 0, emit(*out, stdout, &cfg, filterOwned(renamed, cfg.OwnedFiles))
}

func mergeTools(dst, src map[string]json.RawMessage) map[string]json.RawMessage {
	if dst == nil {
		dst = map[string]json.RawMessage{}
	}
	names := make([]string, 0, len(src))
	for name := range src {
		names = append(names, name)
	}
	sort.Strings(names)
	for _, name := range names {
		dst[name] = src[name]
	}
	return dst
}

func Main(tool string, analyzers []*analysis.Analyzer, configure func([]string) error, args []string, stdout, stderr io.Writer) int {
	if os.Getenv(ChildEnv) != "" {
		runChild(analyzers, configure)
		return 0
	}
	code, err := run(tool, analyzers, configure, args, stdout, stderr)
	if err != nil {
		fmt.Fprintln(stderr, tool+":", err)
		if code == 0 {
			code = 1
		}
	}
	return code
}
