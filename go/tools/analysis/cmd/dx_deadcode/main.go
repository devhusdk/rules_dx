// Command dx_deadcode runs golang.org/x/tools/cmd/deadcode over a staged GOPATH tree.
//
// deadcode loads packages through `go list`. The quality shard copies the target's Go closure
// into dx-deadcode/gopath/src/<importpath>, so the old GOPATH mode needs no go.mod and no
// network. The first argument is the deadcode binary; `go` is found on PATH. The Go team keeps GO111MODULE=off supported (golang/go#60915).
package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
)

const configPath = "dx-deadcode/config.json"

type config struct {
	Packages  []string          `json:"packages"`
	Test      bool              `json:"test"`
	Workspace []string          `json:"workspace"`
	Files     map[string]string `json:"files"`
	GOOS      string            `json:"goos"`
	GOARCH    string            `json:"goarch"`
}

type position struct {
	File      string
	Line, Col int
}

type function struct {
	Name      string
	Position  position
	Generated bool
	Marker    bool
}

type pkg struct {
	Name  string
	Path  string
	Funcs []function
}

func main() {
	os.Exit(run(os.Stdout, os.Stderr))
}

func run(stdout, stderr io.Writer) int {
	if len(os.Args) != 2 {
		fmt.Fprintln(stderr, "usage: dx_deadcode <deadcode binary>")
		return 2
	}
	raw, err := os.ReadFile(configPath)
	if err != nil {
		fmt.Fprintln(stderr, "dx_deadcode:", err)
		return 2
	}
	var cfg config
	if err := json.Unmarshal(raw, &cfg); err != nil {
		fmt.Fprintln(stderr, "dx_deadcode:", err)
		return 2
	}
	root, err := os.Getwd()
	if err != nil {
		fmt.Fprintln(stderr, "dx_deadcode:", err)
		return 2
	}
	scratch, err := os.MkdirTemp("", "dx-deadcode-")
	if err != nil {
		fmt.Fprintln(stderr, "dx_deadcode:", err)
		return 2
	}
	defer os.RemoveAll(scratch)
	gopath := filepath.Join(root, "dx-deadcode", "gopath")
	cmd := exec.Command(os.Args[1], args(cfg)...)
	cmd.Dir = gopath
	cmd.Env = environment(cfg, gopath, scratch)
	var out bytes.Buffer
	cmd.Stdout = &out
	cmd.Stderr = stderr
	runErr := cmd.Run()
	code := 0
	if runErr != nil {
		exit, ok := runErr.(*exec.ExitError)
		if !ok {
			fmt.Fprintln(stderr, "dx_deadcode:", runErr)
			return 2
		}
		code = exit.ExitCode()
	}
	if code != 0 {
		return code
	}
	rewritten, err := rewrite(out.Bytes(), gopath, cfg.Files)
	if err != nil {
		fmt.Fprintln(stderr, "dx_deadcode:", err)
		return 2
	}
	stdout.Write(rewritten)
	return 0
}

func args(cfg config) []string {
	argv := []string{"-json"}
	if cfg.Test {
		argv = append(argv, "-test")
	}
	argv = append(argv, "-filter="+filter(cfg.Workspace))
	return append(argv, cfg.Packages...)
}

// filter matches exactly the workspace's import paths.
func filter(workspace []string) string {
	quoted := make([]string, len(workspace))
	for i, path := range workspace {
		quoted[i] = regexp.QuoteMeta(path)
	}
	return "^(" + strings.Join(quoted, "|") + ")$"
}

func environment(cfg config, gopath, scratch string) []string {
	env := []string{
		"GOPATH=" + gopath,
		"GO111MODULE=off",
		"CGO_ENABLED=0",
		"GOWORK=off",
		"GOTOOLCHAIN=local",
		"GOFLAGS=",
		"GOCACHE=" + filepath.Join(scratch, "cache"),
		"HOME=" + scratch,
		"TMPDIR=" + scratch,
		"PATH=" + os.Getenv("PATH"),
	}
	if cfg.GOOS != "" {
		env = append(env, "GOOS="+cfg.GOOS)
	}
	if cfg.GOARCH != "" {
		env = append(env, "GOARCH="+cfg.GOARCH)
	}
	return env
}

// rewrite maps each position to its workspace path and drops functions in other files.
func rewrite(data []byte, gopath string, files map[string]string) ([]byte, error) {
	var packages []pkg
	if len(bytes.TrimSpace(data)) > 0 {
		if err := json.Unmarshal(data, &packages); err != nil {
			return nil, err
		}
	}
	kept := []pkg{}
	for _, p := range packages {
		var funcs []function
		for _, fn := range p.Funcs {
			workspace, ok := files[staged(fn.Position.File, gopath)]
			if !ok {
				continue
			}
			fn.Position.File = workspace
			funcs = append(funcs, fn)
		}
		if len(funcs) > 0 {
			p.Funcs = funcs
			kept = append(kept, p)
		}
	}
	return json.MarshalIndent(kept, "", "\t")
}

// staged turns a position file into its path below gopath/src.
func staged(file, gopath string) string {
	file = filepath.ToSlash(file)
	src := filepath.ToSlash(gopath) + "/src/"
	file = strings.TrimPrefix(file, src)
	return strings.TrimPrefix(file, "src/")
}
