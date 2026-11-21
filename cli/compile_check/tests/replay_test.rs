//! `dx compile_check` replays steps that are this binary started with `--fake`.

use dx_testing::{path_text, scratch_dir};
use std::path::{Path, PathBuf};

/// Prints argv[0], then the file at argv[2] if one is named, to stderr and exits with argv[1].
fn fake(args: &[String]) -> ! {
    eprintln!("{}", args[0]);
    if let Some(file) = args.get(2).filter(|f| !f.is_empty()) {
        match std::fs::read(file) {
            Ok(bytes) => {
                use std::io::Write;
                std::io::stderr().write_all(&bytes).expect("write stderr");
            }
            Err(e) => eprintln!("{file}: {e}"),
        }
    }
    std::process::exit(args[1].parse().expect("exit code"));
}

struct Env {
    me: String,
    tmp: PathBuf,
}

impl Env {
    fn run(&self, name: &str, options: &[&str], steps: &[&[&str]]) -> i32 {
        let mut args = vec![
            self.path(&format!("{name}.log")),
            self.path(&format!("{name}.exit")),
        ];
        args.extend(options.iter().map(|&o| o.to_owned()));
        args.push("--".to_owned());
        for (i, step) in steps.iter().enumerate() {
            if i > 0 {
                args.push("--dx-next--".to_owned());
            }
            args.push(self.me.clone());
            args.push("--fake".to_owned());
            args.extend(step.iter().map(|&s| s.to_owned()));
        }
        dx_compile_check::run_process(&args)
    }

    fn read(&self, name: &str) -> String {
        read(&self.tmp.join(name))
    }

    fn path(&self, name: &str) -> String {
        path_text(&self.tmp.join(name))
    }
}

fn read(path: &Path) -> String {
    let text =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    text.trim_end_matches('\n').to_owned()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--fake") {
        fake(&args[2..]);
    }
    let env = Env {
        me: path_text(&std::env::current_exe().expect("current exe")),
        tmp: scratch_dir("compile_check"),
    };

    assert_eq!(
        env.run("a", &[], &[&["one", "0"], &["two", "3"]]),
        0,
        "the replay exits 0"
    );
    assert_eq!(env.read("a.exit"), "3", "status is the last exit code");
    assert_eq!(env.read("a.log"), "one\ntwo", "both steps log");

    env.run("b", &[], &[&["one", "1"], &["two", "0"]]);
    assert_eq!(env.read("b.exit"), "1", "first failure is the status");
    assert_eq!(env.read("b.log"), "one", "later steps do not run");

    env.run(
        "c",
        &[
            "--alias",
            "bazel-out/k8/bin/p/__obazl/M.ml",
            "p/m.ml",
            "--alias",
            "a.b[1]*",
            "X",
        ],
        &[&[
            "File \"bazel-out/k8/bin/p/__obazl/M.ml\", line 2, a.b[1]* and aXb",
            "1",
        ]],
    );
    assert_eq!(
        env.read("c.log"),
        "File \"p/m.ml\", line 2, X and aXb",
        "aliases are fixed strings"
    );

    std::fs::write(env.tmp.join("flags"), "-a\n-b\n").expect("write flags");
    let flags = env.path("flags");
    let flags_out = env.path("flags.out");
    assert_eq!(
        env.run(
            "d",
            &[
                "--append", "4", &flags_out, "--line", "-c", "--line", "-d e"
            ],
            &[&["x", "0", &flags]]
        ),
        0,
        "append exits 0"
    );
    assert_eq!(env.read("flags"), "-a\n-b", "the original is unchanged");
    assert_eq!(
        env.read("flags.out"),
        "-a\n-b\n-c\n-d e",
        "append adds one line per flag"
    );
    assert_eq!(
        env.read("d.log"),
        "x\n-a\n-b\n-c\n-d e",
        "the step reads the copy"
    );

    let none_out = env.path("none.out");
    let missing = env.path("missing");
    env.run(
        "e",
        &["--append", "4", &none_out, "--line", "-c"],
        &[&["x", "0", &missing]],
    );
    assert_eq!(env.read("e.exit"), "2", "missing append source is an error");
    assert_eq!(
        env.read("e.log"),
        "compile_check: no file at argv[4] to append to"
    );

    assert_eq!(
        env.run("f", &["--bogus"], &[&["x", "0"]]),
        2,
        "unknown option must fail"
    );

    let unwritable = env.path("no/such/dir/g.log");
    assert_eq!(
        dx_compile_check::run_process(&[
            unwritable,
            env.path("g.exit"),
            "--".to_owned(),
            env.me.clone(),
            "--fake".to_owned(),
            "x".to_owned(),
            "0".to_owned(),
        ]),
        1,
        "an unwritable log fails the replay"
    );
    println!("PASS");
}
