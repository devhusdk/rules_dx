use std::path::{Path, PathBuf};
use std::process::Command;

struct Driver {
    name: &'static str,
    env: &'static str,
    include: &'static str,
    output: &'static str,
    warnings: &'static str,
    werror: &'static str,
    c_std: &'static str,
}

const DRIVERS: &[Driver] = &[
    Driver {
        name: "clang",
        env: "DX_CLANG",
        include: "-I",
        output: "-o{object}",
        warnings: "-Wall",
        werror: "-Werror",
        c_std: "-std=c17",
    },
    Driver {
        name: "clang-cl",
        env: "DX_CLANGCL",
        include: "/I",
        output: "/Fo{object}",
        warnings: "/W4",
        werror: "/WX",
        c_std: "/std:c17",
    },
];

const CLANGXX_ENV: &str = "DX_CLANGXX";
const CLANGCL_ENV: &str = "DX_CLANGCL";

const CLEAN_C: &str = "cc/tests/fixtures/copts/strict.c";
const WARNING_C: &str = "cc/tests/fixtures/warnings/warn.c";
const CXX23: &str = "cc/tests/fixtures/copts/cxx23.cc";

struct Compile {
    code: Option<i32>,
    text: String,
}

impl Compile {
    fn succeeded(&self) -> bool {
        self.code == Some(0)
    }

    fn describe(&self) -> &str {
        &self.text
    }
}

fn binary(env: &str) -> String {
    std::env::var(env).unwrap_or_else(|_| panic!("{env}"))
}

fn workspace_root() -> PathBuf {
    let srcdir = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR");
    let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE");
    Path::new(&srcdir).join(workspace)
}

fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dx-cc-driver-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn compile(
    bin: &str,
    include: &str,
    output_flag: &str,
    tag: &str,
    source: &str,
    flags: &[&str],
) -> Compile {
    let root = workspace_root();
    let object = scratch().join(format!("{tag}.o"));
    let mut argv: Vec<String> = vec![bin.to_owned()];
    for flag in flags {
        argv.push((*flag).to_owned());
    }
    argv.push(include.to_owned());
    argv.push(root.display().to_string());
    argv.push("-c".to_owned());
    argv.push(root.join(source).display().to_string());
    argv.push(output_flag.replace("{object}", &object.display().to_string()));
    let output = Command::new(&argv[0])
        .args(&argv[1..])
        .output()
        .expect("the qualified driver runs");
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    Compile {
        code: output.status.code(),
        text,
    }
}

fn compile_c(driver: &Driver, tag: &str, source: &str, flags: &[&str]) -> Compile {
    compile(
        &binary(driver.env),
        driver.include,
        driver.output,
        tag,
        source,
        flags,
    )
}

fn compile_clangxx(tag: &str, source: &str, flags: &[&str]) -> Compile {
    compile(&binary(CLANGXX_ENV), "-I", "-o{object}", tag, source, flags)
}

fn compile_clangcl(tag: &str, source: &str, flags: &[&str]) -> Compile {
    compile(
        &binary(CLANGCL_ENV),
        "/I",
        "/Fo{object}",
        tag,
        source,
        flags,
    )
}

#[test]
fn clean_c_compiles_under_each_qualified_driver_policy() {
    for driver in DRIVERS {
        let got = compile_c(
            driver,
            &format!("clean-{}", driver.name),
            CLEAN_C,
            &[driver.warnings, driver.werror, driver.c_std],
        );
        assert!(
            got.succeeded(),
            "{} must compile plain C with warnings as errors: {}",
            driver.name,
            got.describe()
        );
    }
}

#[test]
fn a_warning_fails_the_compile_under_each_qualified_driver() {
    for driver in DRIVERS {
        let got = compile_c(
            driver,
            &format!("warn-{}", driver.name),
            WARNING_C,
            &[driver.warnings, driver.werror],
        );
        assert!(
            !got.succeeded(),
            "{} must fail a warning under warnings-as-errors: {}",
            driver.name,
            got.describe()
        );
        assert!(
            got.text.contains("unused variable"),
            "{} must report the warning it escalated: {}",
            driver.name,
            got.describe()
        );
    }
}

#[test]
fn an_existing_warning_flag_alone_carries_no_policy() {
    for driver in DRIVERS {
        let got = compile_c(
            driver,
            &format!("flag-{}", driver.name),
            WARNING_C,
            &[driver.werror],
        );
        assert!(
            got.succeeded(),
            "{} must still need its own warning level: {}",
            driver.name,
            got.describe()
        );
    }
}

#[test]
fn the_requested_cxx_standard_reaches_the_cxx_driver() {
    let got = compile_clangxx("cxx23", CXX23, &["-Wall", "-Werror", "-std=c++23"]);
    assert!(
        got.succeeded(),
        "clang++ must compile C++23 under the warning policy: {}",
        got.describe()
    );
    let got = compile_clangxx("cxx17", CXX23, &["-Wall", "-Werror", "-std=c++17"]);
    assert!(
        !got.succeeded(),
        "the C++23 fixture must fail when the standard is downgraded: {}",
        got.describe()
    );
    assert!(
        got.text
            .contains("the requested C++ standard was translated down"),
        "the downgrade diagnostic must name the translated standard: {}",
        got.describe()
    );
}

#[test]
fn clang_cl_qualifies_its_own_standard_spelling() {
    let got = compile_clangcl("latest", CXX23, &["/W4", "/WX", "/std:c++latest"]);
    assert!(
        got.succeeded(),
        "clang-cl must compile C++23 as /std:c++latest: {}",
        got.describe()
    );
    let got = compile_clangcl("cxx20", CXX23, &["/W4", "/WX", "/std:c++20"]);
    assert!(
        !got.succeeded(),
        "the C++23 fixture must fail on clang-cl before C++23: {}",
        got.describe()
    );
    assert!(
        got.text
            .contains("multidimensional subscript operator is unavailable"),
        "clang-cl must reject C++20 for a C++23 fixture: {}",
        got.describe()
    );
}

#[test]
fn clang_cl_rejects_the_unix_standard_spelling() {
    let got = compile_clangcl("gnu-std", CXX23, &["-std=c++23"]);
    assert!(
        got.text.contains("unknown argument ignored in clang-cl"),
        "clang-cl must reject -std=c++23: {}",
        got.describe()
    );
    let got = compile_clangcl("msvc-23", CXX23, &["/std:c++23"]);
    assert!(
        got.text.contains("argument unused during compilation"),
        "clang-cl must report /std:c++23 as unused: {}",
        got.describe()
    );
    assert!(
        got.text
            .contains("the requested C++ standard was translated down"),
        "an ignored /std:c++23 must leave the standard downgraded: {}",
        got.describe()
    );
}

#[test]
fn clang_cl_keeps_cxx_only_options_out_of_c_compilation() {
    let driver = &DRIVERS[1];
    let got = compile_c(driver, "cl-c17", CLEAN_C, &[driver.c_std]);
    assert!(
        got.succeeded(),
        "clang-cl must compile plain C as /std:c17: {}",
        got.describe()
    );
    let got = compile_c(driver, "cl-gnu-std", CLEAN_C, &["-std=c17"]);
    assert!(
        got.text.contains("unknown argument ignored in clang-cl"),
        "clang-cl must reject -std=c17 for a C compile: {}",
        got.describe()
    );
}
