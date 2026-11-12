//! compile_check_exec.sh runs under busybox sh with this binary as the step it replays.

use dx_testing::{env_file, path_text, scratch_dir};
use std::path::{Path, PathBuf};
use std::process::Command;

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
    sh: PathBuf,
    exec_sh: String,
    me: String,
    tmp: PathBuf,
}

impl Env {
    fn run(&self, name: &str, options: &[&str], steps: &[&[&str]]) -> bool {
        let mut cmd = Command::new(&self.sh);
        cmd.arg(&self.exec_sh)
            .arg(self.tmp.join(format!("{name}.log")))
            .arg(self.tmp.join(format!("{name}.exit")))
            .args(options)
            .arg("--");
        for (i, step) in steps.iter().enumerate() {
            if i > 0 {
                cmd.arg("--dx-next--");
            }
            cmd.arg(&self.me).arg("--fake").args(*step);
        }
        cmd.status().expect("run compile_check_exec.sh").success()
    }

    fn read(&self, name: &str) -> String {
        read(&self.tmp.join(name))
    }

    fn path(&self, name: &str) -> String {
        path_text(&self.tmp.join(name))
    }
}

fn read(path: &Path) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    text.trim_end_matches('\n').to_owned()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--fake") {
        fake(&args[2..]);
    }
    let env = Env {
        sh: env_file("BUSYBOX_SH"),
        exec_sh: path_text(&env_file("COMPILE_CHECK_EXEC")),
        me: path_text(&std::env::current_exe().expect("current exe")),
        tmp: scratch_dir("compile_check_exec"),
    };

    assert!(env.run("a", &[], &[&["one", "0"], &["two", "3"]]), "helper exits 0");
    assert_eq!(env.read("a.exit"), "3", "status is the last exit code");
    assert_eq!(env.read("a.log"), "one\ntwo", "both steps log");

    env.run("b", &[], &[&["one", "1"], &["two", "0"]]);
    assert_eq!(env.read("b.exit"), "1", "first failure is the status");
    assert_eq!(env.read("b.log"), "one", "later steps do not run");

    env.run(
        "c",
        &["--alias", "bazel-out/k8/bin/p/__obazl/M.ml", "p/m.ml", "--alias", "a.b[1]*", "X"],
        &[&["File \"bazel-out/k8/bin/p/__obazl/M.ml\", line 2, a.b[1]* and aXb", "1"]],
    );
    assert_eq!(env.read("c.log"), "File \"p/m.ml\", line 2, X and aXb", "aliases are fixed strings");
    env.run("c2", &["--alias", "dir with space/x", "y"], &[&["in dir with space/x:1", "1"]]);
    assert_eq!(env.read("c2.log"), "in y:1", "aliases handle spaces");

    env.run(
        "c3",
        &["--alias", "bazel-out/k8/bin/_ocaml/foo.ml", "R&D/foo.ml"],
        &[&["File \"bazel-out/k8/bin/_ocaml/foo.ml\", line 1: Error", "1"]],
    );
    assert_eq!(env.read("c3.log"), "File \"R&D/foo.ml\", line 1: Error", "ampersand in alias");

    std::fs::write(env.tmp.join("flags"), "-a\n-b\n").expect("write flags");
    let flags = env.path("flags");
    let flags_out = env.path("flags.out");
    assert!(
        env.run("d", &["--append", "4", &flags_out, "--line", "-c", "--line", "-d e"], &[&["x", "0", &flags]]),
        "append exits 0"
    );
    assert_eq!(env.read("flags"), "-a\n-b", "the original is unchanged");
    assert_eq!(env.read("flags.out"), "-a\n-b\n-c\n-d e", "append adds one line per flag");

    let none_out = env.path("none.out");
    let missing = env.path("missing");
    env.run("e", &["--append", "4", &none_out, "--line", "-c"], &[&["x", "0", &missing]]);
    assert_eq!(env.read("e.exit"), "2", "missing append source is an error");

    assert!(!env.run("f", &["--bogus"], &[&["x", "0"]]), "unknown option must fail");
    println!("PASS");
}
