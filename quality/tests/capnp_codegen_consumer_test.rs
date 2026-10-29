use dx_nested::{self as nested, Case};
use dx_testing::{read, rules_dx_source, write};

const MONEY: &str = "@0xa1b2c3d4e5f60711;\n\nusing Java = import \"/capnp/java.capnp\";\n$Java.package(\"shop.money\");\n$Java.outerClassname(\"MoneySchema\");\n\nstruct Money {\n  cents @0 :Int64;\n}\n";

const ITEM: &str = "@0xa1b2c3d4e5f60722;\n\nusing Cxx = import \"/capnp/c++.capnp\";\n$Cxx.namespace(\"shop::items\");\n\nusing Java = import \"/capnp/java.capnp\";\n$Java.package(\"shop.items\");\n$Java.outerClassname(\"ItemSchema\");\n\nusing Money = import \"/money/money.capnp\";\n\nstruct Item {\n  price @0 :Money.Money;\n  name @1 :Text;\n}\n";

const UNDECLARED: &str = "@0xa1b2c3d4e5f60733;\n\nusing Hidden = import \"/hidden/hidden.capnp\";\n\nstruct Lost {\n  hidden @0 :Hidden.Hidden;\n}\n";

const HIDDEN: &str = "@0xa1b2c3d4e5f60744;\n\nstruct Hidden {\n  id @0 :Int32;\n}\n";

const MODULE: &str = r#"module(name = "consumer")

bazel_dep(name = "rules_cc", version = "0.2.27")
bazel_dep(name = "rules_java", version = "9.9.0")
bazel_dep(name = "rules_rust", version = "0.75.0")
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

const SHOP_RUST_BUILD: &str = r#"load("@rules_dx//capnproto/rules:codegen.bzl", "capnp_rust_library")
load("@rules_dx//capnproto/rules:defs.bzl", "capnp_library")
load("@rules_rust//rust:defs.bzl", "rust_test")

capnp_library(
    name = "item",
    srcs = ["item.capnp"],
    deps = ["//money"],
)

capnp_rust_library(
    name = "item_rs",
    library = "//shop:item",
    crate_name = "shop_items",
    deps = ["//money:money_rs"],
)

rust_test(
    name = "rs_app",
    srcs = ["app.rs"],
    deps = [
        ":item_rs",
        "//money:money_rs",
        "@rules_dx//capnproto:runtime_rust",
    ],
)
"#;

const MONEY_RUST_BUILD: &str = r#"load("@rules_dx//capnproto/rules:codegen.bzl", "capnp_rust_library")
load("@rules_dx//capnproto/rules:defs.bzl", "capnp_library")

capnp_library(
    name = "money",
    srcs = ["money.capnp"],
    visibility = ["//visibility:public"],
)

capnp_rust_library(
    name = "money_rs",
    library = ":money",
    visibility = ["//visibility:public"],
)
"#;

const APP_RS: &str = r#"use shop_items::item_capnp::item;

#[test]
fn nested_package_types_resolve_through_the_dependency_crate() {
    let mut message = capnp::message::Builder::new_default();
    let mut builder = message.init_root::<item::Builder>();
    builder.reborrow().init_price().set_cents(250);
    builder.set_name("tea");
    let price: money_rs::money_capnp::money::Reader = builder.into_reader().get_price().unwrap();
    assert_eq!(price.get_cents(), 250);
}
"#;

const SHOP_JAVA_BUILD: &str = r#"load("@rules_dx//capnproto/rules:codegen.bzl", "capnp_java_library")
load("@rules_dx//capnproto/rules:defs.bzl", "capnp_library")
load("@rules_java//java:java_binary.bzl", "java_binary")

capnp_library(
    name = "item",
    srcs = ["item.capnp"],
    deps = ["//money"],
)

capnp_java_library(
    name = "item_java",
    deps = [":item", "//money"],
)

java_binary(
    name = "java_app",
    srcs = ["JavaApp.java"],
    main_class = "JavaApp",
    deps = [":item_java"],
)
"#;

const JAVA_APP: &str = r#"import org.capnproto.MessageBuilder;
import shop.items.ItemSchema;

public final class JavaApp {
  public static void main(String[] args) {
    MessageBuilder message = new MessageBuilder();
    ItemSchema.Item.Builder item = message.initRoot(ItemSchema.Item.factory);
    item.initPrice().setCents(250);
    if (item.getPrice().getCents() != 250) {
      throw new AssertionError("price");
    }
  }
}
"#;

const NO_JAVA_PACKAGE: &str = "@0xa1b2c3d4e5f60755;\n\nstruct Bare {\n  id @0 :Int32;\n}\n";

const BARE_BUILD: &str = r#"load("@rules_dx//capnproto/rules:codegen.bzl", "capnp_java_library")
load("@rules_dx//capnproto/rules:defs.bzl", "capnp_library")

capnp_library(
    name = "bare",
    srcs = ["bare.capnp"],
)

capnp_java_library(
    name = "bare_java",
    deps = [":bare"],
)
"#;

const LOST_RS_BUILD: &str = r#"load("@rules_dx//capnproto/rules:codegen.bzl", "capnp_rust_library")
load("@rules_dx//capnproto/rules:defs.bzl", "capnp_library")

capnp_library(
    name = "lost",
    srcs = ["lost.capnp"],
)

capnp_rust_library(
    name = "lost_rs",
    library = ":lost",
)
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
fn a_rust_consumer_links_a_dependency_crate_with_a_nondefault_crate_name() {
    let root = consumer();
    write(&root.join("money/BUILD.bazel"), MONEY_RUST_BUILD);
    write(&root.join("shop/BUILD.bazel"), SHOP_RUST_BUILD);
    write(&root.join("shop/app.rs"), APP_RS);
    let out = root.bazel(&["test", "//shop:rs_app"]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn a_java_consumer_builds_across_packages() {
    let root = consumer();
    write(&root.join("shop/BUILD.bazel"), SHOP_JAVA_BUILD);
    write(&root.join("shop/JavaApp.java"), JAVA_APP);
    let out = root.bazel(&["build", "//shop:java_app"]);
    assert_eq!(out.status.code(), Some(0), "{}", out.combined());
}

#[test]
fn a_schema_without_a_java_package_fails_with_the_plugin_message() {
    let root = consumer();
    write(&root.join("bare/BUILD.bazel"), BARE_BUILD);
    write(&root.join("bare/bare.capnp"), NO_JAVA_PACKAGE);
    let out = root.bazel(&["build", "//bare:bare_java"]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("no Java package name found"),
        "{}",
        out.combined()
    );
}

#[test]
fn an_undeclared_import_fails_the_rust_path() {
    let root = consumer();
    write(&root.join("lost/BUILD.bazel"), LOST_RS_BUILD);
    write(&root.join("lost/lost.capnp"), UNDECLARED);
    let out = root.bazel(&["build", "//lost:lost_rs"]);
    assert_ne!(out.status.code(), Some(0), "{}", out.combined());
    assert!(
        out.combined().contains("hidden.capnp"),
        "{}",
        out.combined()
    );
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
