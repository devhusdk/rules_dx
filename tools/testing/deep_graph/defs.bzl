"""Linear library chains that //tools/testing:analysis_scaling_check analyzes with the dx check and docs aspects."""

load("@bazel_skylib//rules:write_file.bzl", "write_file")
load("//capnproto:defs.bzl", "capnproto_library")
load("//go:defs.bzl", "go_library")
load("//java:defs.bzl", "java_library")
load("//julia:defs.bzl", "julia_library")
load("//kotlin:defs.bzl", "kotlin_library")
load("//ruby:defs.bzl", "ruby_library")
load("//typescript:defs.bzl", "typescript_library")

_ENABLED = select({
    "//tools/testing/deep_graph:enabled": [],
    "//conditions:default": ["@platforms//:incompatible"],
})

def _source(name, out, lines):
    write_file(
        name = name + "_src",
        out = out,
        content = lines,
        testonly = True,
        target_compatible_with = _ENABLED,
    )

def _go(i, dep, prefix):
    name = "go_%d" % i
    _source(name, name + ".go", ["package go_%d" % i])
    go_library(
        name = name,
        testonly = True,
        srcs = [name + ".go"],
        importpath = "%s/go_%d" % (prefix, i),
        deps = [dep] if dep else [],
        target_compatible_with = _ENABLED,
    )

def _java(i, dep):
    name = "Java%d" % i
    _source(name, name + ".java", ["package deep_graph;", "final class %s {}" % name])
    java_library(
        name = name,
        testonly = True,
        srcs = [name + ".java"],
        deps = [dep] if dep else [],
        target_compatible_with = _ENABLED,
    )

def _kotlin(i, dep):
    name = "Kotlin%d" % i
    _source(name, name + ".kt", ["package deep_graph", "class %s" % name])
    kotlin_library(
        name = name,
        testonly = True,
        srcs = [name + ".kt"],
        deps = [dep] if dep else [],
        target_compatible_with = _ENABLED,
    )

def _typescript(i, dep):
    name = "ts_%d" % i
    _source(name, name + ".ts", ["export const value%d: number = %d;" % (i, i)])
    typescript_library(
        name = name,
        testonly = True,
        srcs = [name + ".ts"],
        declaration = True,
        transpiler = "tsc",
        tsconfig = "//tools/testing/deep_graph:tsconfig",
        deps = [dep] if dep else [],
        target_compatible_with = _ENABLED,
    )

def _capnp(i, dep):
    name = "capnp_%d" % i
    _source(name, name + ".capnp", ["@0x%s;" % _capnp_id(i)])
    capnproto_library(
        name = name,
        testonly = True,
        srcs = [name + ".capnp"],
        deps = [dep] if dep else [],
        target_compatible_with = _ENABLED,
    )

def _capnp_id(i):
    digits = "0123456789abcdef"
    out = ""
    for _ in range(15):
        out = digits[i % 16] + out
        i //= 16
    return "8" + out

def _julia(i, dep):
    name = "Julia%d" % i
    _source(name, "src/%s.jl" % name, ["module %s" % name, "end"])
    julia_library(
        name = name,
        testonly = True,
        srcs = ["src/%s.jl" % name],
        deps = [dep] if dep else [],
        target_compatible_with = _ENABLED,
    )

def _ruby(i, dep):
    name = "ruby_%d" % i
    _source(name, name + ".rb", ["# frozen_string_literal: true", "", "VALUE_%d = %d" % (i, i)])
    ruby_library(
        name = name,
        testonly = True,
        srcs = [name + ".rb"],
        deps = [dep] if dep else [],
        target_compatible_with = _ENABLED,
    )

def deep_graph_chains(depth):
    """Declares one linear chain of `depth` libraries per language, each node depending on the one before.

    The targets are incompatible unless --//tools/testing/deep_graph:enabled is set.

    Args:
      depth: number of libraries in each chain.
    """
    prefix = "github.com/devhusdk/rules_dx/" + native.package_name()
    previous = {}
    for i in range(1, depth + 1):
        _go(i, previous.get("go"), prefix)
        _java(i, previous.get("java"))
        _kotlin(i, previous.get("kotlin"))
        _typescript(i, previous.get("ts"))
        _capnp(i, previous.get("capnp"))
        _julia(i, previous.get("julia"))
        _ruby(i, previous.get("ruby"))
        previous = {
            "capnp": ":capnp_%d" % i,
            "go": ":go_%d" % i,
            "java": ":Java%d" % i,
            "julia": ":Julia%d" % i,
            "kotlin": ":Kotlin%d" % i,
            "ruby": ":ruby_%d" % i,
            "ts": ":ts_%d" % i,
        }
