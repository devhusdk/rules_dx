"""Wrapper conformance tests for PHP."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true")
load(":defs.bzl", "PHP_COMPATIBLE_WITH", "PHP_DEFERRED_SKIP_TAGS", "PHP_SOURCE_SPECS", "php_compatible_with", "php_deferred_tags", "php_kwargs", "php_quality_srcs")
load(":php.bzl", "PHP_SOURCE_EXTS", "autoloader_suffix", "ini_flags", "launcher_script", "runfiles_path", "shell_quote", "staged_path")

def _file(short_path):
    return struct(short_path = short_path)

_SELECT = select({"//conditions:default": []})

def php_wrapper_contract_tests(name):
    """Instantiates PHP wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["php"],
        extra_checks = [
            expect_equal("one php class owns every source", PHP_SOURCE_SPECS, [("php", ["php"])]),
            expect_equal("wrappers accept only PHP sources", PHP_SOURCE_EXTS, [".php"]),
            expect_equal("main leads the owned sources", php_quality_srcs("main.php", ["src/A.php"]), ["main.php", "src/A.php"]),
            expect_equal("main listed in srcs is owned once", php_quality_srcs("main.php", ["main.php", "src/A.php"]), ["main.php", "src/A.php"]),
            expect_equal("autoload files are owned once", php_quality_srcs(None, ["a.php"], ["a.php", "f.php"]), ["a.php", "f.php"]),
            expect_equal("no main and no srcs owns nothing", php_quality_srcs(None, None), []),
            expect_equal("the deferred class skips quality", PHP_DEFERRED_SKIP_TAGS, ["no-format"]),
            expect_equal("user tags stay first", php_deferred_tags(["manual", "no-lint"]), ["manual", "no-lint", "no-format"]),
            expect_equal("targets build on the pinned cells", type(PHP_COMPATIBLE_WITH), "select"),
            expect_equal("caller constraints join the cells select", type(php_compatible_with(["//x:y"])), "select"),
            expect_equal("no caller constraints keep the cells select", type(php_compatible_with(None)), "select"),
            expect_equal("a select() constraint passes through", type(php_compatible_with(_SELECT)), "select"),
            expect_equal("other attrs pass through", php_kwargs({"deps": [":a"], "ini": {"k": "v"}})["ini"], {"k": "v"}),
            expect_equal("workspace sources keep their path", staged_path(_file("app/src/A.php")), "app/src/A.php"),
            expect_equal("external sources stage under external/", staged_path(_file("../dep+/src/B.php")), "external/dep+/src/B.php"),
            expect_equal("suffixes are identifiers", autoloader_suffix(Label("//app/cli:hello-world")), "Dx_main_app_cli_hello_world"),
            expect_equal("root-package suffixes keep the empty package", autoloader_suffix(Label("//:hello")), "Dx_main__hello"),
            expect_equal("ini flags sort by key", ini_flags({"z": "1", "a": "b=c"}), ["-d", "a=b=c", "-d", "z=1"]),
            expect_equal("quotes survive the shell", shell_quote("it's"), "'it'\\''s'"),
            expect_equal("workspace runfiles", runfiles_path("_main", _file("app/x.php")), "_main/app/x.php"),
            expect_equal("external runfiles", runfiles_path("_main", _file("../dep+/x.php")), "dep+/x.php"),
            expect_contains("the launcher runs php without php.ini", launcher_script("php+/php", {"memory_limit": "64M"}, "\"$@\""), 'exec "$runfiles/php+/php" -n \'-d\' \'memory_limit=64M\' "$@"\n'),
            expect_true("the launcher exports the runfiles root", "export RUNFILES_DIR=\"$runfiles\"" in launcher_script("p", {}, "")),
        ],
    )
