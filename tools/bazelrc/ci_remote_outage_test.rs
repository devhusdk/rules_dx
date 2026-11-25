use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

use dx_nested as nested;
use dx_preset::{Layer, render_layer};

const ACTIONS: usize = 32;

const CHAIN: usize = 6;

const CHAIN_BZL: &str = "def _impl(rctx):\n    previous = rctx.read(rctx.attr.previous) if rctx.attr.previous else \"\"\n    rctx.file(\"value.txt\", previous + rctx.attr.value + \"\\n\")\n    rctx.file(\"BUILD.bazel\", \"exports_files([\\\"value.txt\\\"])\\n\")\n    return rctx.repo_metadata(reproducible = True)\n\nchain = repository_rule(\n    implementation = _impl,\n    attrs = {\n        \"previous\": attr.label(),\n        \"value\": attr.string(),\n    },\n)\n";

const STAMP_BZL: &str = "def _impl(ctx):\n    out = ctx.actions.declare_file(ctx.label.name + \".txt\")\n    ctx.actions.run(executable = ctx.executable.tool, arguments = [out.path, ctx.attr.nonce], outputs = [out])\n    return [DefaultInfo(files = depset([out]))]\n\nstamp = rule(\n    implementation = _impl,\n    attrs = {\n        \"nonce\": attr.string(),\n        \"tool\": attr.label(default = \":stamp.sh\", executable = True, cfg = \"exec\", allow_single_file = True),\n    },\n)\n";

const STAMP_SH: &str = "#!/bin/sh\nprintf '%s\\n' \"$2\" > \"$1\"\n";

fn stalled_endpoint() -> (u16, Arc<Mutex<Vec<TcpStream>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("local addr").port();
    let held = Arc::new(Mutex::new(Vec::new()));
    let kept = Arc::clone(&held);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = stream.try_clone().expect("clone stream");
            kept.lock().expect("held streams").push(stream);
            std::thread::spawn(move || {
                let mut buffer = [0_u8; 4096];
                while matches!(reader.read(&mut buffer), Ok(read) if read > 0) {}
            });
        }
    });
    (port, held)
}

#[test]
fn a_stalled_remote_cache_trips_the_breaker_and_the_build_fetches_and_runs_locally() {
    if nested::recording() {
        return;
    }
    let case = nested::case("ci_remote_outage");
    let lock = nested::checked_in_lock("harness");
    case.write("MODULE.bazel.lock", &lock);
    let nonce = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    );
    let mut module = format!(
        "module(name = \"ci_remote_outage\")\n{}\nchain = use_repo_rule(\"//:chain.bzl\", \"chain\")\n",
        nested::locked_deps(&lock)
    );
    // Each repository reads the previous one, so the remote cache lookups run one after another.
    for index in 0..CHAIN {
        let previous = match index {
            0 => String::new(),
            _ => format!(", previous = \"@c{}//:value.txt\"", index - 1),
        };
        module.push_str(&format!(
            "chain(name = \"c{index}\", value = \"{nonce}-c{index}\"{previous})\n"
        ));
    }
    case.write("MODULE.bazel", &module);
    case.write(
        "performance.bazelrc",
        &render_layer(Layer::Performance, "rules_dx"),
    );
    case.write(
        ".bazelrc",
        "import %workspace%/performance.bazelrc\nstartup --experimental_remote_repo_contents_cache\n",
    );
    case.write("chain.bzl", CHAIN_BZL);
    case.write("stamp.bzl", STAMP_BZL);
    case.write("stamp.sh", STAMP_SH);
    let script = case.join("stamp.sh");
    let mut permissions = std::fs::metadata(&script).expect("stat stamp.sh").permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(&script, permissions).expect("chmod stamp.sh");
    let mut build = format!(
        "load(\":stamp.bzl\", \"stamp\")\n\nexports_files([\"stamp.sh\"])\n\nfilegroup(name = \"chain\", srcs = [\"@c{}//:value.txt\"])\n",
        CHAIN - 1
    );
    for index in 0..ACTIONS {
        build.push_str(&format!(
            "\nstamp(name = \"s{index}\", nonce = \"{nonce}-{index}\")\n"
        ));
    }
    case.write("BUILD.bazel", &build);

    let (port, _held) = stalled_endpoint();
    let remote = format!("--remote_cache=grpcs://127.0.0.1:{port}");
    let run = case.bazel(&[
        "build",
        "--config=ci",
        &remote,
        "--disk_cache=",
        "--color=no",
        "--curses=no",
        "//:all",
    ]);
    let output = run.combined();
    assert!(run.status.success(), "{output}");
    assert!(
        output.contains("Call not executed due to a high failure rate."),
        "the circuit breaker never tripped:\n{output}"
    );
    assert!(
        output.contains("Remote repo contents cache lookup failed for"),
        "no repository went through the remote repo contents cache:\n{output}"
    );
    for index in 0..ACTIONS {
        let stamp = dx_testing::read_text(&case.join(format!("bazel-bin/s{index}.txt")));
        assert_eq!(stamp.trim(), format!("{nonce}-{index}"));
    }
}
