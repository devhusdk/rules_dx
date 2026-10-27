"""BCR submission tooling for rules_dx."""

load("//deploy/rules:defs.bzl", "dx_deployment")
load("//deploy/rules:launcher.bzl", "rlocation_path")
load("//deploy/rules:validate.bzl", "genrule_arg")
load("//rust/rules:defs.bzl", "rust_binary")

def bcr_source_error(module_name, version):
    """Validates the BCR module name + version pair."""
    if module_name != "rules_dx":
        return ("bcr: invalid module '" + str(module_name) +
                "': want 'rules_dx'")
    if type(version) != "string" or version == "":
        return ("bcr: invalid version '" + str(version) +
                "': want SemVer MAJOR.MINOR.PATCH (for example '1.0.0')")
    parts = version.split(".")
    if len(parts) != 3:
        return ("bcr: invalid version '" + version +
                "': want SemVer MAJOR.MINOR.PATCH (for example '1.0.0')")
    for part in parts:
        if part == "" or not part[0].isdigit():
            return ("bcr: invalid version '" + version +
                    "': want SemVer MAJOR.MINOR.PATCH (for example '1.0.0')")
    return ""

def _bcr_launcher_impl(ctx):
    """Writes the owner-gated BCR deploy launcher Rust source."""
    rlocs = []
    for target in ctx.attr.inputs:
        info = target[DefaultInfo]
        fl = info.files.to_list()
        if len(fl) != 1:
            fail("bcr_check " + str(ctx.label) + ": input " +
                 str(target.label) + " provides " + str(len(fl)) +
                 " files, want exactly one")
        f = fl[0]
        rloc = rlocation_path(ctx, f)
        for banned in ["\"", "\\", "\n"]:
            if banned in rloc:
                fail("bcr_check " + str(ctx.label) + ": rlocation '" + rloc +
                     "' is not launcher-safe (quotes, backslashes, newlines)")
        rlocs.append(rloc)
    for value in [ctx.attr.module_name, ctx.attr.version]:
        for banned in ["\"", "\\", "\n"]:
            if banned in value:
                fail("bcr_check " + str(ctx.label) + ": value '" + value +
                     "' is not launcher-safe")
    rloc_list = ", ".join(["\"" + r + "\"" for r in rlocs])
    launcher = ctx.actions.declare_file(ctx.label.name + ".rs")
    ctx.actions.write(
        output = launcher,
        content = """// Deploy launcher for `bcr_check`. Generated. Do not edit.
fn run() -> i32 {
    const MODULE: &str = \"""" + ctx.attr.module_name + """\";
    const VERSION: &str = \"""" + ctx.attr.version + """\";
    const INPUT_RLOCS: &[&str] = &[""" + rloc_list + """];
    if std::env::args_os().len() > 1 {
        eprintln!("bcr: this deploy target takes no extra args; the submission is exactly the pinned inputs");
        return 1;
    }
    let dry = std::env::var("BCR_DRY_RUN").unwrap_or_default() == "1";
    let approved = std::env::var("BCR_APPROVE").unwrap_or_default() == "1";
    let runfiles = match runfiles::Runfiles::create() {
        Ok(runfiles) => runfiles,
        Err(error) => {
            eprintln!("bcr: cannot load runfiles: {error}");
            return 1;
        }
    };
    let mut inputs = Vec::with_capacity(INPUT_RLOCS.len());
    for rloc in INPUT_RLOCS {
        match runfiles.rlocation(rloc) {
            Some(path) => inputs.push(path.to_string_lossy().into_owned()),
            None => {
                eprintln!("bcr: runfile not found for '{rloc}'");
                return 1;
            }
        }
    }
    match dx_release_tools::bcr_run(MODULE, VERSION, &inputs, dry, approved) {
        Ok(text) => {
            print!("{text}");
            0
        }
        Err(diagnostic) => {
            eprintln!("{diagnostic}");
            1
        }
    }
}
fn main() {
    std::process::exit(run());
}
""",
    )
    return [DefaultInfo(files = depset([launcher]))]

_bcr_launcher = rule(
    implementation = _bcr_launcher_impl,
    doc = "Writes the owner-gated BCR deploy launcher Rust source.",
    attrs = {
        "inputs": attr.label_list(
            doc = "Targets whose single output file is submitted with the module.",
            mandatory = True,
        ),
        "module_name": attr.string(
            doc = "Bazel module name being submitted, which must be rules_dx.",
            mandatory = True,
        ),
        "version": attr.string(
            doc = "Dotted numeric version of the module being submitted.",
            mandatory = True,
        ),
    },
)

def bcr_check(name, module_name = "rules_dx", version = "0.0.0", inputs = [], profile = "release"):
    """Creates an owner-gated BCR shape-check deploy target."""
    src_err = bcr_source_error(module_name, version)
    if src_err != "":
        fail(src_err + " (in " + native.package_name() + ":" + name + ")")

    native.genrule(
        name = name + "_source",
        outs = [name + ".source.json"],
        tools = ["//deploy/release:release_tool"],
        cmd = "$(location //deploy/release:release_tool) bcr-source $(OUTS) " + genrule_arg(module_name) + " " + genrule_arg(version),
    )

    program_target = name + "_program"
    launcher_target = program_target + "_launcher"
    all_inputs = [":" + name + "_source"] + inputs
    _bcr_launcher(
        name = launcher_target,
        inputs = all_inputs,
        module_name = module_name,
        version = version,
    )

    rust_binary(
        name = program_target,
        srcs = [":" + launcher_target],
        crate_name = program_target.replace("-", "_"),
        data = all_inputs,
        edition = "2021",
        deps = [
            "//deploy/release:dx_release_tools",
            "@rules_rust//rust/runfiles",
        ],
    )

    dx_deployment(
        name = name,
        deploy = ":" + program_target,
        profile = profile,
    )
