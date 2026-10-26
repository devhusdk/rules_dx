//! The consumer `MODULE.bazel` stanza that pins the CUDA toolkit and its host compiler.
//!
//! rules_cuda reads `cuda.*` tags only from the root module, and toolchains_llvm's `llvm`
//! extension is root-only too, so a consumer has to repeat the pins that rules_dx itself uses.
//! The text mirrors the root `MODULE.bazel`, `modules/cuda.bzl` and `modules/versions.bzl`;
//! the drift tests below compare against them.
//! The library components (cuBLAS, cuFFT, cuRAND) are declared for every consumer but Bazel
//! fetches an archive only when a target uses it.
//!
//! When the host compiler stops being LLVM 22 only (issue 3316), delete the host-compiler part
//! in `host_compiler_stanza`; nothing else depends on it.

pub const RULES_CUDA_VERSION: &str = "0.3.0";

pub const TOOLCHAINS_LLVM_VERSION: &str = "1.12.0";

pub const CUDA_HOST_LLVM_VERSION: &str = "22.1.8";

pub const CUDA_VERSION: &str = "13.4";

const CUDA_REDIST: &str = "https://developer.download.nvidia.com/compute/cuda/redist/";

/// Host triple directory of the redistributable archives. Only this table names the host.
const CUDA_HOST: &str = "linux-x86_64";

const CUDA_HOST_SYSROOT_URL: &str = "https://commondatastorage.googleapis.com/chrome-linux-sysroot/toolchain/2202c161310ffde63729f29d27fe7bb24a0bc540/debian_stretch_amd64_sysroot.tar.xz";

const CUDA_HOST_SYSROOT_SHA256: &str =
    "84656a6df544ecef62169cfe3ab6e41bb4346a62d3ba2a045dc5a0a2ecea94a3";

/// `(component, redist directory, archive stem, version, sha256)`.
pub const CUDA_COMPONENTS: [(&str, &str, &str, &str, &str); 8] = [
    (
        "cccl",
        "cccl",
        "cccl",
        "13.3.4.3.1",
        "6b7516074f42f80dd3feb9c11318c8e1c5ab56cdd9f18fff4c5ace2e7e0a8209",
    ),
    (
        "crt",
        "cuda_crt",
        "cuda_crt",
        "13.4.92",
        "c969e61ded12dbf0cf20ace72867f1622ec0e8ea1e1d305e27761012025f7aaa",
    ),
    (
        "cublas",
        "libcublas",
        "libcublas",
        "13.8.0.4",
        "bd6ffcce561672da6abb928214a6694f392741e7d1e2d7dfde0da4bd651706b6",
    ),
    (
        "cudart",
        "cuda_cudart",
        "cuda_cudart",
        "13.4.92",
        "0ac5dbc538d04e9983bc493b410cce4b459e1ee9f5f6654b6464ef7b3e14a8b5",
    ),
    (
        "cufft",
        "libcufft",
        "libcufft",
        "12.4.0.43",
        "f7f8ac2cc67714989a3c8140cda39b1b7ed7d462a491e1ad3f53adb777f5070f",
    ),
    (
        "curand",
        "libcurand",
        "libcurand",
        "10.4.4.72",
        "6f725f9187dc5f675308b830f786c163f5b1b499d86986f94fb60aa701c13c0b",
    ),
    (
        "nvcc",
        "cuda_nvcc",
        "cuda_nvcc",
        "13.4.92",
        "60998f40cc9df5826b2a846e2dcc328133fbc5ef539398ce0a21cf5dcac7193d",
    ),
    (
        "nvvm",
        "libnvvm",
        "libnvvm",
        "13.4.92",
        "0e619cf3d5b27f7c812665599ca9761fc296e56129b24a85d822a5d8a1a3bd75",
    ),
];

fn toolkit_stanza() -> String {
    let mut out =
        String::from("cuda = use_extension(\"@rules_cuda//cuda:extensions.bzl\", \"toolchain\")\n");
    for (component, dir, stem, version, sha256) in CUDA_COMPONENTS {
        let strip_prefix = format!("{stem}-{CUDA_HOST}-{version}-archive");
        out.push_str(&format!(
            "cuda.component(\n    name = \"cuda_{component}\",\n    component_name = \"{component}\",\n    sha256 = \"{sha256}\",\n    strip_prefix = \"{strip_prefix}\",\n    url = \"{CUDA_REDIST}{dir}/{CUDA_HOST}/{strip_prefix}.tar.xz\",\n    version = \"{version}\",\n)\n"
        ));
    }
    out.push_str("cuda.toolkit(\n    name = \"cuda\",\n    components_mapping = {\n");
    for (component, ..) in CUDA_COMPONENTS {
        out.push_str(&format!(
            "        \"{component}\": \"@cuda_{component}\",\n"
        ));
    }
    out.push_str(&format!(
        "    }},\n    nvcc_version = \"{CUDA_VERSION}\",\n    version = \"{CUDA_VERSION}\",\n)\nuse_repo(cuda, \"cuda\")\n"
    ));
    out
}

/// The nvcc host compiler: LLVM 22, registered ahead of the consumer's own C++ toolchain.
fn host_compiler_stanza() -> String {
    format!(
        "cuda_host_sysroot = use_repo_rule(\"@toolchains_llvm//toolchain:sysroot.bzl\", \"sysroot\")\n\ncuda_host_sysroot(\n    name = \"cuda_host_sysroot\",\n    sha256 = \"{CUDA_HOST_SYSROOT_SHA256}\",\n    urls = [\"{CUDA_HOST_SYSROOT_URL}\"],\n)\n\n# The canonical label is deliberate: toolchains_llvm writes target_settings into a generated\n# repository as text, so an apparent @rules_dx label would not resolve there.\ncuda_host_llvm = use_extension(\"@toolchains_llvm//toolchain/extensions:llvm.bzl\", \"llvm\")\ncuda_host_llvm.toolchain(\n    name = \"cuda_host_llvm\",\n    llvm_version = \"{CUDA_HOST_LLVM_VERSION}\",\n    target_settings = {{\"\": [\"@@rules_dx+//cuda/toolchain:host_compiler_enabled\"]}},\n)\ncuda_host_llvm.sysroot(\n    name = \"cuda_host_llvm\",\n    label = \"@cuda_host_sysroot//sysroot\",\n    targets = [\"linux-x86_64\"],\n)\nuse_repo(cuda_host_llvm, \"cuda_host_llvm\")\n\nregister_toolchains(\"@cuda_host_llvm//:all\")\n"
    )
}

/// Renders the consumer stanza. `dx_repo` is the repo name the consumer gave rules_dx.
///
/// Register it before the consumer's own C++ toolchain: if `@llvm//toolchain:all` wins,
/// nvcc gets a host compiler it does not support.
pub fn render_cuda_module_stanza(dx_repo: &str) -> String {
    format!(
        "bazel_dep(name = \"rules_cuda\", version = \"{RULES_CUDA_VERSION}\")\nbazel_dep(name = \"toolchains_llvm\", version = \"{TOOLCHAINS_LLVM_VERSION}\")\n\n{toolkit}\n{host}\n# The CUDA Toolkit is licensed under the NVIDIA EULA: https://docs.nvidia.com/cuda/eula/index.html\n# The next line accepts it for this build; remove it to decline (CUDA targets then fail to build).\ncuda_license = use_extension(\"@{dx_repo}//cuda/toolchain:repos.bzl\", \"cuda_license\")\ncuda_license.license(accept = [\"cuda-eula\"])\n",
        toolkit = toolkit_stanza(),
        host = host_compiler_stanza(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_text(path: &str) -> String {
        let source = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR is set under Bazel");
        let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE is set under Bazel");
        std::fs::read_to_string(std::path::Path::new(&source).join(workspace).join(path))
            .expect("file ships as test data")
    }

    #[test]
    fn toolkit_block_is_verbatim_in_the_root_module() {
        let module = workspace_text("MODULE.bazel");
        assert!(
            module.contains(&toolkit_stanza()),
            "the CUDA toolkit stanza drifts from MODULE.bazel"
        );
    }

    #[test]
    fn host_compiler_block_matches_the_root_module() {
        let module = workspace_text("MODULE.bazel");
        let stanza = host_compiler_stanza();
        let sysroot = stanza
            .split("\n# The canonical label")
            .next()
            .expect("sysroot part");
        assert!(module.contains(sysroot), "sysroot drifts from MODULE.bazel");
        assert!(module.contains(&format!("llvm_version = \"{CUDA_HOST_LLVM_VERSION}\"")));
        assert!(module.contains("label = \"@cuda_host_sysroot//sysroot\""));
        assert!(module.contains("targets = [\"linux-x86_64\"]"));
        assert!(module.contains("\"@@//cuda/toolchain:host_compiler_enabled\""));
    }

    #[test]
    fn versions_match_the_toolchain_metadata() {
        let versions = workspace_text("modules/versions.bzl");
        for (key, value) in [
            ("RULES_CUDA_VERSION", RULES_CUDA_VERSION),
            ("TOOLCHAINS_LLVM_VERSION", TOOLCHAINS_LLVM_VERSION),
            ("CUDA_HOST_LLVM_VERSION", CUDA_HOST_LLVM_VERSION),
            ("CUDA_VERSION", CUDA_VERSION),
        ] {
            assert!(
                versions.contains(&format!("{key} = \"{value}\"")),
                "{key} drifts from modules/versions.bzl"
            );
        }
        let module = workspace_text("MODULE.bazel");
        assert!(module.contains(&format!(
            "bazel_dep(name = \"rules_cuda\", version = \"{RULES_CUDA_VERSION}\")"
        )));
        assert!(module.contains(&format!(
            "bazel_dep(name = \"toolchains_llvm\", version = \"{TOOLCHAINS_LLVM_VERSION}\")"
        )));
    }

    #[test]
    fn components_and_sysroot_match_modules_cuda() {
        let cuda = workspace_text("modules/cuda.bzl");
        for (component, dir, stem, version, sha256) in CUDA_COMPONENTS {
            let archive = format!("{stem}-{CUDA_HOST}-{version}-archive");
            for needle in [
                format!("\"{component}\": struct("),
                format!("version = \"{version}\""),
                format!("sha256 = \"{sha256}\""),
                format!("strip_prefix = \"{archive}\""),
                format!("_REDIST + \"{dir}/{CUDA_HOST}/{archive}.tar.xz\""),
            ] {
                assert!(
                    cuda.contains(&needle),
                    "{component}: {needle} not in cuda.bzl"
                );
            }
        }
        assert!(cuda.contains(&format!("url = \"{CUDA_HOST_SYSROOT_URL}\"")));
        assert!(cuda.contains(&format!("sha256 = \"{CUDA_HOST_SYSROOT_SHA256}\"")));
    }

    #[test]
    fn stanza_parses_and_is_not_dev_only() {
        let text = render_cuda_module_stanza("rules_dx");
        let module = dx_module_bazel::ModuleFile::parse(&text).expect("stanza parses");
        assert_eq!(
            module.bazel_dep_version("rules_cuda").expect("dep").value,
            RULES_CUDA_VERSION
        );
        assert_eq!(
            module
                .bazel_dep_version("toolchains_llvm")
                .expect("dep")
                .value,
            TOOLCHAINS_LLVM_VERSION
        );
        assert!(!text.contains("dev_dependency"));
        assert!(text.contains("cuda.toolkit("));
        assert!(text.contains("accept = [\"cuda-eula\"]"));
        assert!(text.contains("https://docs.nvidia.com/cuda/eula/index.html"));
        assert!(text.contains("remove it to decline"));
    }

    #[test]
    fn host_compiler_is_registered_after_the_toolkit_and_the_license_last() {
        let text = render_cuda_module_stanza("rules_dx");
        let toolkit = text.find("use_repo(cuda, \"cuda\")").expect("toolkit");
        let register = text
            .find("register_toolchains(\"@cuda_host_llvm//:all\")")
            .expect("register");
        let license = text.find("cuda_license.license").expect("license");
        assert!(toolkit < register && register < license);
    }

    #[test]
    fn renamed_dx_repo_changes_only_the_license_extension_label() {
        let plain = render_cuda_module_stanza("rules_dx");
        let renamed = render_cuda_module_stanza("dx");
        assert_eq!(
            plain.replace("@rules_dx//cuda/toolchain", "@dx//cuda/toolchain"),
            renamed
        );
        assert!(renamed.contains("@@rules_dx+//cuda/toolchain:host_compiler_enabled"));
    }
}
