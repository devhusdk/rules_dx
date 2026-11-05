load("@rules_cc//cc:defs.bzl", "cc_binary")
load("@rules_py//py:defs.bzl", "py_binary")

py_binary(
    name = "bin_to_c_source",
    srcs = ["src/bin-to-c-source.py"],
    main = "src/bin-to-c-source.py",
)

genrule(
    name = "version",
    outs = ["version.c"],
    cmd = "echo 'const char *kcov_version = \"43\";' > $@",
)

genrule(
    name = "html_data",
    srcs = glob(["data/**"]),
    outs = ["html-data-files.cc"],
    cmd = " ".join([
        "$(location :bin_to_c_source)",
        "$(location data/bcov.css) css_text",
        "$(location data/amber.png) icon_amber",
        "$(location data/glass.png) icon_glass",
        "$(location data/source-file.html) source_file_text",
        "$(location data/index.html) index_text",
        "$(location data/js/handlebars.js) handlebars_text",
        "$(location data/js/kcov.js) kcov_text",
        "$(location data/js/jquery.min.js) jquery_text",
        "$(location data/js/jquery.tablesorter.min.js) tablesorter_text",
        "$(location data/js/jquery.tablesorter.widgets.min.js) tablesorter_widgets_text",
        "$(location data/tablesorter-theme.css) tablesorter_theme_text",
        "> $@",
    ]),
    tools = [":bin_to_c_source"],
)

# kcov's system mode, bash and python engines are not built. main.cc still
# links the embedded system library, so it gets a one byte stand-in.
genrule(
    name = "system_library_stub",
    outs = ["system-library-stub.cc"],
    cmd = "printf x > stub.bin && $(location :bin_to_c_source) stub.bin kcov_system_library > $@",
    tools = [":bin_to_c_source"],
)

cc_binary(
    name = "kcov",
    srcs = [
        "src/capabilities.cc",
        "src/collector.cc",
        "src/configuration.cc",
        "src/dummy-solib-handler.cc",
        "src/engine-factory.cc",
        "src/engines/kernel-engine.cc",
        "src/engines/ptrace.cc",
        "src/engines/ptrace_linux.cc",
        "src/engines/system-mode-file-format.cc",
        "src/filter.cc",
        "src/main.cc",
        "src/merge-file-parser.cc",
        "src/output-handler.cc",
        "src/parser-manager.cc",
        "src/parsers/dummy-disassembler.cc",
        "src/parsers/dwarf.cc",
        "src/parsers/elf-parser.cc",
        "src/parsers/elf.cc",
        "src/reporter.cc",
        "src/source-file-cache.cc",
        "src/system-mode/file-data.cc",
        "src/utils.cc",
        "src/writers/cobertura-writer.cc",
        "src/writers/codecov-writer.cc",
        "src/writers/dummy-coveralls-writer.cc",
        "src/writers/html-writer.cc",
        "src/writers/json-writer.cc",
        "src/writers/sonarqube-xml-writer.cc",
        "src/writers/writer-base.cc",
        ":html_data",
        ":system_library_stub",
        ":version",
    ] + glob(["src/**/*.hh", "src/include/**"]),
    copts = [
        "-w",
        "-include",
        "errno.h",
        "-D_GLIBCXX_USE_NANOSLEEP",
        "-DKCOV_LIBRARY_PREFIX=\\\"/tmp\\\"",
        "-DKCOV_HAS_LIBBFD=0",
        "-DKCOV_LIBFD_DISASM_STYLED=0",
    ],
    cxxopts = ["-std=c++17"],
    includes = ["src/include"],
    target_compatible_with = ["@platforms//os:linux"],
    visibility = ["//visibility:public"],
    deps = [
        "@elfutils//:libdw",
        "@elfutils//:libelf",
        "@zlib",
    ],
)
