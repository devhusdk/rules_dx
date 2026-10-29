use dx_nested::{self as nested, Case};
use dx_testing::{read, rules_dx_source, write};

const MONEY: &str = "@0xa1b2c3d4e5f60711;\n\nstruct Money {\n  cents @0 :Int64;\n}\n";

const ITEM: &str = "@0xa1b2c3d4e5f60722;\n\nusing Cxx = import \"/capnp/c++.capnp\";\n$Cxx.namespace(\"shop::items\");\n\nusing Money = import \"/money/money.capnp\";\n\nstruct Item {\n  price @0 :Money.Money;\n  name @1 :Text;\n}\n";

const UNDECLARED: &str = "@0xa1b2c3d4e5f60733;\n\nusing Hidden = import \"/hidden/hidden.capnp\";\n\nstruct Lost {\n  hidden @0 :Hidden.Hidden;\n}\n";

const HIDDEN: &str = "@0xa1b2c3d4e5f60744;\n\nstruct Hidden {\n  id @0 :Int32;\n}\n";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_cc", version = "0.2.27")
bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")
"#;

const MONEY_BUILD: &str = r#"load("@rules_dx//capnproto/rules:defs.bzl", "capnp_library")

capnp_library(
    name = "money",
    srcs = ["money.capnp"],
    visibility = ["//visibility:public"],
)
"#;

const HIDDEN_BUILD: &str = r#"load("@rules_dx//capnproto/rules:defs.bzl", "capnp_library")

capnp_library(
    name = "hidden",
    srcs = ["hidden.capnp"],
    visibility = ["//visibility:public"],
)
"#;

const SHOP_BUILD: &str = r#"load("@rules_cc//cc:defs.bzl", "cc_binary")
load("@rules_dx//capnproto/rules:codegen.bzl", "capnp_cc_library")
load("@rules_dx//capnproto/rules:defs.bzl", "capnp_library")

capnp_library(
    name = "item",
    srcs = ["item.capnp"],
    deps = ["//money"],
)

capnp_cc_library(
    name = "item_cc",
    deps = [":item", "//money"],
)

cc_binary(
    name = "app",
    srcs = ["app.cc"],
    copts = ["-std=c++17"],
    deps = [":item_cc"],
)
"#;

const APP: &str = r#"#include <capnp/message.h>
#include "money/money.capnp.h"
#include "shop/item.capnp.h"

int main() {
  capnp::MallocMessageBuilder builder;
  auto item = builder.initRoot<shop::items::Item>();
  item.initPrice().setCents(250);
  item.setName("tea");
  return item.getPrice().getCents() == 250 ? 0 : 1;
}
"#;

const LOST_BUILD: &str = r#"load("@rules_dx//capnproto/rules:codegen.bzl", "capnp_cc_library")
load("@rules_dx//capnproto/rules:defs.bzl", "capnp_library")

capnp_library(
    name = "lost",
    srcs = ["lost.capnp"],
)

capnp_cc_library(
    name = "lost_cc",
    deps = [":lost"],
)
"#;

fn consumer() -> Case {
    let rules = rules_dx_source();
    let root = nested::case("capnp-codegen-consumer");
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
    write(&root.join("money/money.capnp"), MONEY);
    write(&root.join("hidden/BUILD.bazel"), HIDDEN_BUILD);
    write(&root.join("hidden/hidden.capnp"), HIDDEN);
    write(&root.join("shop/BUILD.bazel"), SHOP_BUILD);
    write(&root.join("shop/item.capnp"), ITEM);
    write(&root.join("shop/app.cc"), APP);
    root
}

#[test]
fn a_multi_package_library_builds_a_cpp_consumer_with_a_custom_namespace() {
    let root = consumer();
    let out = root.bazel(&["build", "//shop:app"]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn an_import_outside_the_declared_deps_fails_the_build() {
    let root = consumer();
    write(&root.join("lost/BUILD.bazel"), LOST_BUILD);
    write(&root.join("lost/lost.capnp"), UNDECLARED);
    let out = root.bazel(&["build", "//lost:lost_cc"]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("hidden.capnp"),
        "{}",
        out.combined()
    );
}
