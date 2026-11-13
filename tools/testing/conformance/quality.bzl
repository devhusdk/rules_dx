"""Generated from the lint and format entries of every <lang>/lang.json quality.aspects."""

load("//ansible/private/quality:ansible.bzl", "real_ansible_lint_aspect")
load("//bicep/private/quality:bicep.bzl", "real_bicep_format_aspect", "real_bicep_lint_aspect")
load("//cloudformation/private/quality:cloudformation.bzl", "real_cloudformation_lint_aspect")
load("//cmake/private/quality:cmake.bzl", "real_cmake_format_aspect", "real_cmake_lint_aspect")
load("//core/quality:families/cc.bzl", "real_cc_format_aspect", "real_cc_lint_aspect")
load("//core/quality:families/generation.bzl", "real_avro_lint_aspect", "real_capnp_lint_aspect", "real_flatbuffers_lint_aspect", "real_protobuf_format_aspect", "real_protobuf_lint_aspect", "real_thrift_lint_aspect")
load("//core/quality/testdata:config_subject.bzl", "config_subject_rule")
load("//d/private/quality:d.bzl", "real_d_format_aspect", "real_d_lint_aspect")
load("//dart/private/quality:dart.bzl", "real_dart_format_aspect", "real_dart_lint_aspect")
load("//dockerfile/private/quality:dockerfile.bzl", "real_dockerfile_lint_aspect")
load("//dotnet/private/quality:dotnet.bzl", "real_dotnet_format_aspect", "real_dotnet_lint_aspect")
load("//erlang/private/quality:beam.bzl", "real_elixir_format_aspect", "real_elixir_lint_aspect", "real_erlang_format_aspect", "real_erlang_lint_aspect")
load("//fortran/private/quality:fortran.bzl", "real_fortran_format_aspect", "real_fortran_lint_aspect")
load("//go/private/quality:go.bzl", "real_go_format_aspect", "real_go_lint_aspect")
load("//haskell/private/quality:haskell.bzl", "real_haskell_format_aspect", "real_haskell_lint_aspect")
load("//java/private/quality:jvm.bzl", "real_jvm_format_aspect", "real_jvm_lint_aspect", "real_pkl_lint_aspect", "real_scala_format_aspect", "real_scala_lint_aspect")
load("//javascript/private/quality:js.bzl", "real_js_format_aspect", "real_js_lint_aspect")
load("//julia/private/quality:julia.bzl", "real_julia_format_aspect", "real_julia_lint_aspect")
load("//kubernetes/private/quality:kubernetes.bzl", "real_kubernetes_format_aspect", "real_kubernetes_lint_aspect")
load("//lua/private/quality:lua.bzl", "real_lua_format_aspect", "real_lua_lint_aspect")
load("//make/private/quality:make.bzl", "real_make_format_aspect", "real_make_lint_aspect")
load("//ocaml/private/quality:ocaml.bzl", "real_ocaml_format_aspect", "real_ocaml_lint_aspect")
load("//perl/private/quality:perl.bzl", "real_perl_format_aspect", "real_perl_lint_aspect")
load("//php/private/quality:php.bzl", "real_php_lint_aspect")
load("//powershell/private/quality:powershell.bzl", "real_powershell_format_aspect", "real_powershell_lint_aspect")
load("//python/private/quality:python.bzl", "real_python_format_aspect", "real_python_lint_aspect")
load("//rego/private/quality:rego.bzl", "real_rego_format_aspect", "real_rego_lint_aspect")
load("//ruby/private/quality:ruby.bzl", "real_ruby_format_aspect", "real_ruby_lint_aspect")
load("//rust/private/quality:rust.bzl", "real_rust_format_aspect", "real_rust_lint_aspect")
load("//sql/private/quality:sql.bzl", "real_sql_format_aspect", "real_sql_lint_aspect")
load("//swift/private/quality:swift.bzl", "real_swift_format_aspect", "real_swift_lint_aspect")
load("//terraform/private/quality:terraform.bzl", "real_terraform_format_aspect", "real_terraform_lint_aspect")
load("//zig/private/quality:zig.bzl", "real_zig_format_aspect", "real_zig_lint_aspect")

ansible_lint_subject = config_subject_rule([real_ansible_lint_aspect], [OutputGroupInfo])

api_spec_format_subject = config_subject_rule([real_protobuf_format_aspect], [OutputGroupInfo])

api_spec_lint_subject = config_subject_rule([real_protobuf_lint_aspect], [OutputGroupInfo])

avro_lint_subject = config_subject_rule([real_avro_lint_aspect], [OutputGroupInfo])

bicep_format_subject = config_subject_rule([real_bicep_format_aspect], [OutputGroupInfo])

bicep_lint_subject = config_subject_rule([real_bicep_lint_aspect], [OutputGroupInfo])

capnproto_lint_subject = config_subject_rule([real_capnp_lint_aspect], [OutputGroupInfo])

cc_format_subject = config_subject_rule([real_cc_format_aspect], [OutputGroupInfo])

cc_lint_subject = config_subject_rule([real_cc_lint_aspect], [OutputGroupInfo])

cloudformation_lint_subject = config_subject_rule([real_cloudformation_lint_aspect], [OutputGroupInfo])

cmake_format_subject = config_subject_rule([real_cmake_format_aspect], [OutputGroupInfo])

cmake_lint_subject = config_subject_rule([real_cmake_lint_aspect], [OutputGroupInfo])

d_format_subject = config_subject_rule([real_d_format_aspect], [OutputGroupInfo])

d_lint_subject = config_subject_rule([real_d_lint_aspect], [OutputGroupInfo])

dart_format_subject = config_subject_rule([real_dart_format_aspect], [OutputGroupInfo])

dart_lint_subject = config_subject_rule([real_dart_lint_aspect], [OutputGroupInfo])

dockerfile_lint_subject = config_subject_rule([real_dockerfile_lint_aspect], [OutputGroupInfo])

dotnet_format_subject = config_subject_rule([real_dotnet_format_aspect], [OutputGroupInfo])

dotnet_lint_subject = config_subject_rule([real_dotnet_lint_aspect], [OutputGroupInfo])

elixir_format_subject = config_subject_rule([real_elixir_format_aspect], [OutputGroupInfo])

elixir_lint_subject = config_subject_rule([real_elixir_lint_aspect], [OutputGroupInfo])

erlang_format_subject = config_subject_rule([real_erlang_format_aspect], [OutputGroupInfo])

erlang_lint_subject = config_subject_rule([real_erlang_lint_aspect], [OutputGroupInfo])

flatbuffers_lint_subject = config_subject_rule([real_flatbuffers_lint_aspect], [OutputGroupInfo])

fortran_format_subject = config_subject_rule([real_fortran_format_aspect], [OutputGroupInfo])

fortran_lint_subject = config_subject_rule([real_fortran_lint_aspect], [OutputGroupInfo])

go_format_subject = config_subject_rule([real_go_format_aspect], [OutputGroupInfo])

go_lint_subject = config_subject_rule([real_go_lint_aspect], [OutputGroupInfo])

haskell_format_subject = config_subject_rule([real_haskell_format_aspect], [OutputGroupInfo])

haskell_lint_subject = config_subject_rule([real_haskell_lint_aspect], [OutputGroupInfo])

java_format_subject = config_subject_rule([real_jvm_format_aspect], [OutputGroupInfo])

java_lint_subject = config_subject_rule([real_jvm_lint_aspect, real_pkl_lint_aspect], [OutputGroupInfo])

javascript_format_subject = config_subject_rule([real_js_format_aspect], [OutputGroupInfo])

javascript_lint_subject = config_subject_rule([real_js_lint_aspect], [OutputGroupInfo])

julia_format_subject = config_subject_rule([real_julia_format_aspect], [OutputGroupInfo])

julia_lint_subject = config_subject_rule([real_julia_lint_aspect], [OutputGroupInfo])

kubernetes_format_subject = config_subject_rule([real_kubernetes_format_aspect], [OutputGroupInfo])

kubernetes_lint_subject = config_subject_rule([real_kubernetes_lint_aspect], [OutputGroupInfo])

lua_format_subject = config_subject_rule([real_lua_format_aspect], [OutputGroupInfo])

lua_lint_subject = config_subject_rule([real_lua_lint_aspect], [OutputGroupInfo])

make_format_subject = config_subject_rule([real_make_format_aspect], [OutputGroupInfo])

make_lint_subject = config_subject_rule([real_make_lint_aspect], [OutputGroupInfo])

ocaml_format_subject = config_subject_rule([real_ocaml_format_aspect], [OutputGroupInfo])

ocaml_lint_subject = config_subject_rule([real_ocaml_lint_aspect], [OutputGroupInfo])

perl_format_subject = config_subject_rule([real_perl_format_aspect], [OutputGroupInfo])

perl_lint_subject = config_subject_rule([real_perl_lint_aspect], [OutputGroupInfo])

php_lint_subject = config_subject_rule([real_php_lint_aspect], [OutputGroupInfo])

powershell_format_subject = config_subject_rule([real_powershell_format_aspect], [OutputGroupInfo])

powershell_lint_subject = config_subject_rule([real_powershell_lint_aspect], [OutputGroupInfo])

python_format_subject = config_subject_rule([real_python_format_aspect], [OutputGroupInfo])

python_lint_subject = config_subject_rule([real_python_lint_aspect], [OutputGroupInfo])

rego_format_subject = config_subject_rule([real_rego_format_aspect], [OutputGroupInfo])

rego_lint_subject = config_subject_rule([real_rego_lint_aspect], [OutputGroupInfo])

ruby_format_subject = config_subject_rule([real_ruby_format_aspect], [OutputGroupInfo])

ruby_lint_subject = config_subject_rule([real_ruby_lint_aspect], [OutputGroupInfo])

rust_format_subject = config_subject_rule([real_rust_format_aspect], [OutputGroupInfo])

rust_lint_subject = config_subject_rule([real_rust_lint_aspect], [OutputGroupInfo])

scala_format_subject = config_subject_rule([real_scala_format_aspect], [OutputGroupInfo])

scala_lint_subject = config_subject_rule([real_scala_lint_aspect], [OutputGroupInfo])

sql_format_subject = config_subject_rule([real_sql_format_aspect], [OutputGroupInfo])

sql_lint_subject = config_subject_rule([real_sql_lint_aspect], [OutputGroupInfo])

swift_format_subject = config_subject_rule([real_swift_format_aspect], [OutputGroupInfo])

swift_lint_subject = config_subject_rule([real_swift_lint_aspect], [OutputGroupInfo])

terraform_format_subject = config_subject_rule([real_terraform_format_aspect], [OutputGroupInfo])

terraform_lint_subject = config_subject_rule([real_terraform_lint_aspect], [OutputGroupInfo])

thrift_lint_subject = config_subject_rule([real_thrift_lint_aspect], [OutputGroupInfo])

zig_format_subject = config_subject_rule([real_zig_format_aspect], [OutputGroupInfo])

zig_lint_subject = config_subject_rule([real_zig_lint_aspect], [OutputGroupInfo])

QUALITY_SUBJECTS = {
    "ansible": {
        "lint": ansible_lint_subject,
    },
    "api_spec": {
        "format": api_spec_format_subject,
        "lint": api_spec_lint_subject,
    },
    "avro": {
        "lint": avro_lint_subject,
    },
    "bicep": {
        "format": bicep_format_subject,
        "lint": bicep_lint_subject,
    },
    "capnproto": {
        "lint": capnproto_lint_subject,
    },
    "cc": {
        "format": cc_format_subject,
        "lint": cc_lint_subject,
    },
    "cloudformation": {
        "lint": cloudformation_lint_subject,
    },
    "cmake": {
        "format": cmake_format_subject,
        "lint": cmake_lint_subject,
    },
    "d": {
        "format": d_format_subject,
        "lint": d_lint_subject,
    },
    "dart": {
        "format": dart_format_subject,
        "lint": dart_lint_subject,
    },
    "dockerfile": {
        "lint": dockerfile_lint_subject,
    },
    "dotnet": {
        "format": dotnet_format_subject,
        "lint": dotnet_lint_subject,
    },
    "elixir": {
        "format": elixir_format_subject,
        "lint": elixir_lint_subject,
    },
    "erlang": {
        "format": erlang_format_subject,
        "lint": erlang_lint_subject,
    },
    "flatbuffers": {
        "lint": flatbuffers_lint_subject,
    },
    "fortran": {
        "format": fortran_format_subject,
        "lint": fortran_lint_subject,
    },
    "go": {
        "format": go_format_subject,
        "lint": go_lint_subject,
    },
    "haskell": {
        "format": haskell_format_subject,
        "lint": haskell_lint_subject,
    },
    "java": {
        "format": java_format_subject,
        "lint": java_lint_subject,
    },
    "javascript": {
        "format": javascript_format_subject,
        "lint": javascript_lint_subject,
    },
    "julia": {
        "format": julia_format_subject,
        "lint": julia_lint_subject,
    },
    "kubernetes": {
        "format": kubernetes_format_subject,
        "lint": kubernetes_lint_subject,
    },
    "lua": {
        "format": lua_format_subject,
        "lint": lua_lint_subject,
    },
    "make": {
        "format": make_format_subject,
        "lint": make_lint_subject,
    },
    "ocaml": {
        "format": ocaml_format_subject,
        "lint": ocaml_lint_subject,
    },
    "perl": {
        "format": perl_format_subject,
        "lint": perl_lint_subject,
    },
    "php": {
        "lint": php_lint_subject,
    },
    "powershell": {
        "format": powershell_format_subject,
        "lint": powershell_lint_subject,
    },
    "python": {
        "format": python_format_subject,
        "lint": python_lint_subject,
    },
    "rego": {
        "format": rego_format_subject,
        "lint": rego_lint_subject,
    },
    "ruby": {
        "format": ruby_format_subject,
        "lint": ruby_lint_subject,
    },
    "rust": {
        "format": rust_format_subject,
        "lint": rust_lint_subject,
    },
    "scala": {
        "format": scala_format_subject,
        "lint": scala_lint_subject,
    },
    "sql": {
        "format": sql_format_subject,
        "lint": sql_lint_subject,
    },
    "swift": {
        "format": swift_format_subject,
        "lint": swift_lint_subject,
    },
    "terraform": {
        "format": terraform_format_subject,
        "lint": terraform_lint_subject,
    },
    "thrift": {
        "lint": thrift_lint_subject,
    },
    "zig": {
        "format": zig_format_subject,
        "lint": zig_lint_subject,
    },
}
