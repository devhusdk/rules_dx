use dx_nested::{self as nested, Case};
use dx_testing::{read, rules_dx_source, write};

const COMMON: &str = "namespace shop;\n\ntable Money {\n  cents:long;\n}\n";

const ITEM: &str = "include \"money/money.fbs\";\n\nnamespace shop;\n\ntable Item {\n  price:Money;\n  name:string;\n}\n\nroot_type Item;\n";

const UNDECLARED: &str =
    "include \"hidden/hidden.fbs\";\n\nnamespace shop;\n\ntable Lost {\n  hidden:Hidden;\n}\n";

const HIDDEN: &str = "namespace shop;\n\ntable Hidden {\n  id:int;\n}\n";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_cc", version = "0.2.27")
bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")
"#;

const MONEY_BUILD: &str = r#"load("@rules_dx//flatbuffers/rules:defs.bzl", "flatbuffers_library")

flatbuffers_library(
    name = "money",
    srcs = ["money.fbs"],
    visibility = ["//visibility:public"],
)
"#;

const HIDDEN_BUILD: &str = r#"load("@rules_dx//flatbuffers/rules:defs.bzl", "flatbuffers_library")

flatbuffers_library(
    name = "hidden",
    srcs = ["hidden.fbs"],
    visibility = ["//visibility:public"],
)
"#;

const SHOP_BUILD: &str = r#"load("@rules_cc//cc:defs.bzl", "cc_binary")
load("@rules_dx//flatbuffers/rules:codegen.bzl", "flatbuffers_cc_library")
load("@rules_dx//flatbuffers/rules:defs.bzl", "flatbuffers_library")

flatbuffers_library(
    name = "item",
    srcs = ["item.fbs"],
    deps = ["//money"],
)

flatbuffers_cc_library(
    name = "item_cc",
    deps = [":item", "//money"],
    gen_object_api = GEN_OBJECT_API,
)

cc_binary(
    name = "app",
    srcs = ["app.cc"],
    copts = ["-std=c++17"],
    deps = [":item_cc"],
)
"#;

const APP: &str = r#"#include "flatbuffers/flatbuffers.h"
#include "money/money_generated.h"
#include "shop/item_generated.h"

int main() {
  flatbuffers::FlatBufferBuilder builder;
  auto money = shop::CreateMoney(builder, 250);
  auto name = builder.CreateString("tea");
  builder.Finish(shop::CreateItem(builder, money, name));
  flatbuffers::Verifier verifier(builder.GetBufferPointer(), builder.GetSize());
  return shop::VerifyItemBuffer(verifier) ? 0 : 1;
}
"#;

const LOST_BUILD: &str = r#"load("@rules_dx//flatbuffers/rules:codegen.bzl", "flatbuffers_cc_library")
load("@rules_dx//flatbuffers/rules:defs.bzl", "flatbuffers_library")

flatbuffers_library(
    name = "lost",
    srcs = ["lost.fbs"],
)

flatbuffers_cc_library(
    name = "lost_cc",
    deps = [":lost"],
)
"#;

fn consumer(gen_object_api: bool) -> Case {
    let rules = rules_dx_source();
    let root = nested::case("fbs-codegen-consumer");
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &MODULE.replace("RULES_DX", &rules.display().to_string()),
    );
    write(&root.join(".bazelrc"), "common --enable_bzlmod\n");
    write(&root.join("BUILD.bazel"), "");
    write(&root.join("money/BUILD.bazel"), MONEY_BUILD);
    write(&root.join("money/money.fbs"), COMMON);
    write(&root.join("hidden/BUILD.bazel"), HIDDEN_BUILD);
    write(&root.join("hidden/hidden.fbs"), HIDDEN);
    let flag = if gen_object_api { "True" } else { "False" };
    write(
        &root.join("shop/BUILD.bazel"),
        &SHOP_BUILD.replace("GEN_OBJECT_API", flag),
    );
    write(&root.join("shop/item.fbs"), ITEM);
    write(&root.join("shop/app.cc"), APP);
    root
}

#[test]
fn a_multi_file_library_builds_a_cpp_consumer() {
    let root = consumer(false);
    let out = root.bazel(&["build", "//shop:app"]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn the_object_api_builds_when_requested() {
    let root = consumer(true);
    write(
        &root.join("shop/app.cc"),
        &APP.replace(
            "return shop::VerifyItemBuffer",
            "shop::ItemT unpacked;\n  shop::GetItem(builder.GetBufferPointer())->UnPackTo(&unpacked);\n  return unpacked.name == \"tea\" && shop::VerifyItemBuffer",
        ),
    );
    let out = root.bazel(&["build", "//shop:app"]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn an_include_outside_the_declared_deps_fails_the_build() {
    let root = consumer(false);
    write(&root.join("lost/BUILD.bazel"), LOST_BUILD);
    write(&root.join("lost/lost.fbs"), UNDECLARED);
    let out = root.bazel(&["build", "//lost:lost_cc"]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(out.combined().contains("hidden.fbs"), "{}", out.combined());
}
