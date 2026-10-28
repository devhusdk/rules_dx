package dart

import (
	"context"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"testing"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
)

func generate(t *testing.T, files map[string]string, build string) (language.GenerateResult, []string) {
	t.Helper()
	root := t.TempDir()
	dir := filepath.Join(root, "pkg", "greet")
	var regular []string
	for name, content := range files {
		path := filepath.Join(dir, filepath.FromSlash(name))
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
		if !strings.Contains(name, "/") {
			regular = append(regular, name)
		}
	}
	args := language.GenerateArgs{Config: &config.Config{RepoRoot: root}, Dir: dir, Rel: "pkg/greet", RegularFiles: regular}
	if build != "" {
		f, err := rule.LoadData(filepath.Join(dir, "BUILD.bazel"), "pkg/greet", []byte(build))
		if err != nil {
			t.Fatal(err)
		}
		args.File = f
	}
	lang := NewLanguage()
	lm := lang.(language.LifecycleManager)
	lm.Before(context.Background())
	result := lang.GenerateRules(args)
	lm.AfterResolvingDeps(context.Background())
	var msgs []string
	for _, e := range common.Errors.Take() {
		msgs = append(msgs, e.Message)
	}
	return result, msgs
}

func summary(result language.GenerateResult) []string {
	var out []string
	for _, r := range result.Gen {
		out = append(out, r.Kind()+" "+r.Name()+" main="+r.AttrString("main")+" srcs="+strings.Join(r.AttrStrings("srcs"), ","))
	}
	sort.Strings(out)
	return out
}

const pubspec = "name: greet\ndependencies:\n  collection: any\ndev_dependencies:\n  test: any\n"

func TestGeneratePubLayout(t *testing.T) {
	result, errs := generate(t, map[string]string{
		"pubspec.yaml":         pubspec,
		"lib/greet.dart":       "import 'src/a.dart';\nimport 'package:collection/collection.dart';\nimport 'package:greet/src/a.dart';\n",
		"lib/src/a.dart":       "",
		"bin/greet.dart":       "import 'package:greet/greet.dart';\nimport 'src/b.dart';\n",
		"bin/src/b.dart":       "",
		"bin/tool.dart":        "",
		"test/greet_test.dart": "import 'package:test/test.dart';\nimport 'helper.dart';\n",
		"test/sub/x_test.dart": "",
		"test/helper.dart":     "",
	}, "")
	if len(errs) > 0 {
		t.Fatalf("errors: %v", errs)
	}
	want := []string{
		"dart_binary greet_bin main=bin/greet.dart srcs=bin/src/b.dart",
		"dart_binary tool main=bin/tool.dart srcs=bin/src/b.dart",
		"dart_library greet main= srcs=lib/greet.dart,lib/src/a.dart",
		"dart_test greet_test main=test/greet_test.dart srcs=test/helper.dart",
		"dart_test sub_x_test main=test/sub/x_test.dart srcs=test/helper.dart",
	}
	got := summary(result)
	if strings.Join(got, "\n") != strings.Join(want, "\n") {
		t.Errorf("got\n%s\nwant\n%s", strings.Join(got, "\n"), strings.Join(want, "\n"))
	}
}

func TestGenerateNoPubspec(t *testing.T) {
	result, errs := generate(t, map[string]string{"lib/a.dart": ""}, "")
	if len(result.Gen) != 0 || len(errs) != 0 {
		t.Errorf("got %v %v, want nothing", summary(result), errs)
	}
}

func TestGenerateErrors(t *testing.T) {
	for name, tc := range map[string]struct {
		files map[string]string
		build string
		want  string
	}{
		"undeclared": {map[string]string{"pubspec.yaml": "name: greet\n", "lib/a.dart": "import 'package:collection/x.dart';\n"}, "", "does not declare"},
		"dev in lib": {map[string]string{"pubspec.yaml": pubspec, "lib/a.dart": "import 'package:test/test.dart';\n"}, "", "does not declare"},
		"missing part": {map[string]string{"pubspec.yaml": pubspec, "lib/a.dart": "part 'a.g.dart';\n"}, "", "hand-write the generator rule"},
		"nested build": {map[string]string{"pubspec.yaml": pubspec, "lib/a.dart": "", "lib/sub/BUILD.bazel": "", "lib/sub/b.dart": ""}, "", "own BUILD file"},
		"bad scheme":   {map[string]string{"pubspec.yaml": pubspec, "lib/a.dart": "import 'https://x/y.dart';\n"}, "", "unsupported URI"},
		"syntax":       {map[string]string{"pubspec.yaml": pubspec, "lib/a.dart": "import ;;; @@\n"}, "", "lib/a.dart"},
		"bad pubspec":  {map[string]string{"pubspec.yaml": "version: 1\n", "lib/a.dart": ""}, "", "no name"},
	} {
		t.Run(name, func(t *testing.T) {
			_, errs := generate(t, tc.files, tc.build)
			if !strings.Contains(strings.Join(errs, "\n"), tc.want) {
				t.Errorf("errors = %v, want %q", errs, tc.want)
			}
		})
	}
}

func TestGenerateTestMayUseDevDependency(t *testing.T) {
	_, errs := generate(t, map[string]string{"pubspec.yaml": pubspec, "test/a_test.dart": "import 'package:test/test.dart';\n"}, "")
	if len(errs) != 0 {
		t.Errorf("errors = %v", errs)
	}
}

func TestGeneratePartFromGenrule(t *testing.T) {
	result, errs := generate(t, map[string]string{"pubspec.yaml": pubspec, "lib/a.dart": "part 'a.g.dart';\n"},
		"genrule(name = \"gen\", outs = [\"lib/a.g.dart\"], cmd = \"touch $@\")\n")
	if len(errs) != 0 {
		t.Fatalf("errors = %v", errs)
	}
	if got := summary(result); len(got) != 1 || !strings.Contains(got[0], "lib/a.dart,lib/a.g.dart") {
		t.Errorf("got %v", got)
	}
}

func TestGenerateKeepsUserOwnedAttrs(t *testing.T) {
	result, _ := generate(t, map[string]string{"pubspec.yaml": pubspec, "lib/a.dart": ""},
		"dart_library(name = \"greet\", package_name = \"greet\", srcs = [\"lib/a.dart\"], data = [\"x\"], aspect_hints = [\"//:h\"])\n")
	for _, r := range result.Gen {
		if r.AttrStrings("data") != nil || r.AttrStrings("aspect_hints") != nil {
			t.Errorf("generation sets user-owned attributes on %s", r.Name())
		}
	}
	for _, attr := range []string{"data", "defines", "aspect_hints", "tags"} {
		for kind, info := range dartKinds {
			if info.MergeableAttrs[attr] {
				t.Errorf("%s merges %s, want it user-owned", kind, attr)
			}
		}
	}
}

func TestStaleSweep(t *testing.T) {
	result, _ := generate(t, map[string]string{"pubspec.yaml": pubspec, "lib/a.dart": ""},
		"dart_binary(name = \"old\", main = \"bin/old.dart\")\n")
	if len(result.Empty) != 1 || result.Empty[0].Name() != "old" {
		t.Errorf("empty = %v, want old", result.Empty)
	}
}
