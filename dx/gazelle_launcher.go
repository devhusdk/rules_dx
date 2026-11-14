// Command gazelle_launcher runs the dx gazelle binary from its runfiles without a shell.
package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"

	"github.com/bazelbuild/rules_go/go/runfiles"
)

func main() {
	os.Exit(run())
}

func run() int {
	files, err := runfiles.New()
	if err != nil {
		return fail(err)
	}
	gazelle, err := files.Rlocation(os.Getenv("DX_GAZELLE"))
	if err != nil {
		return fail(err)
	}
	var args []string
	if err := json.Unmarshal([]byte(os.Getenv("DX_GAZELLE_ARGS")), &args); err != nil {
		return fail(fmt.Errorf("DX_GAZELLE_ARGS: %w", err))
	}
	if len(os.Args) > 1 && (os.Args[1] == "fix" || os.Args[1] == "update") {
		args = os.Args[1:]
	} else {
		args = append(args, os.Args[1:]...)
	}
	if config := os.Getenv("DX_GAZELLE_REPO_CONFIG"); config != "" && len(args) > 0 && (args[0] == "fix" || args[0] == "update") {
		path, err := files.Rlocation(config)
		if err != nil {
			return fail(err)
		}
		args = append([]string{args[0], "-bzlmod", "-repo_config", path}, args[1:]...)
	}
	env := append(os.Environ(), files.Env()...)
	if goTool := os.Getenv("DX_GAZELLE_GO"); goTool != "" {
		path, err := files.Rlocation(goTool)
		if err != nil {
			return fail(err)
		}
		env = append(env, "GOROOT="+filepath.Dir(filepath.Dir(path)), "GOTOOLCHAIN=local")
	}
	cmd := exec.Command(gazelle, args...)
	cmd.Env = env
	cmd.Stdin = os.Stdin
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	if err := cmd.Run(); err != nil {
		var exit *exec.ExitError
		if errors.As(err, &exit) {
			return exit.ExitCode()
		}
		return fail(err)
	}
	return 0
}

func fail(err error) int {
	fmt.Fprintf(os.Stderr, "gazelle_launcher: %v\n", err)
	return 1
}
