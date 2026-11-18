"""Native cohort matrix cells."""

load(":runner_matrix_tests.bzl", "matrix_case")

visibility("private")

NATIVE_CASES = [
    matrix_case(
        name = "matrix_c_format_pass",
        capability = "format",
        tool = "clang_format",
        binary = "//core/quality/testdata:llvm_clang_format",
        source_class = "c",
        generated = {
            "matrix/clang_format_clean.c": "// Seed C format fixture.\n#include <stdio.h>\n\nint greet(const char *name) { return 0; }\n",
        },
    ),
    matrix_case(
        name = "matrix_c_format_fail",
        capability = "format",
        tool = "clang_format",
        binary = "//core/quality/testdata:llvm_clang_format",
        source_class = "c",
        generated = {
            "matrix/clang_format_dirty.c": "// Seed C format fixture.\n#include <stdio.h>\n\nint greet( const char*name){printf(\"hello %s\\n\", name);return 0;}\n",
        },
    ),
    matrix_case(
        name = "matrix_cpp_format_pass",
        capability = "format",
        tool = "clang_format",
        binary = "//core/quality/testdata:llvm_clang_format",
        source_class = "cpp",
        generated = {
            "matrix/clang_format_clean.cpp": "// Seed C++ format fixture.\n\nint greet(const char *name) { return 0; }\n",
        },
    ),
    matrix_case(
        name = "matrix_cpp_format_fail",
        capability = "format",
        tool = "clang_format",
        binary = "//core/quality/testdata:llvm_clang_format",
        source_class = "cpp",
        generated = {
            "matrix/clang_format_dirty.cpp": "// Seed C++ format fixture.\n\nint greet( const char*name){return 0;}\n",
        },
    ),
    matrix_case(
        name = "matrix_cuda_format_pass",
        capability = "format",
        tool = "clang_format",
        binary = "//core/quality/testdata:llvm_clang_format",
        source_class = "cuda",
        generated = {
            "matrix/clang_format_clean.cu": "// Seed CUDA format fixture.\n\nint greet(const char *name) { return 0; }\n",
        },
    ),
    matrix_case(
        name = "matrix_cuda_format_fail",
        capability = "format",
        tool = "clang_format",
        binary = "//core/quality/testdata:llvm_clang_format",
        source_class = "cuda",
        generated = {
            "matrix/clang_format_dirty.cu": "// Seed CUDA format fixture.\n\nint greet( const char*name){return 0;}\n",
        },
    ),
    matrix_case(
        name = "matrix_objc_format_pass",
        capability = "format",
        tool = "clang_format",
        binary = "//core/quality/testdata:llvm_clang_format",
        source_class = "objc",
        generated = {
            "matrix/clang_format_clean.m": "// Seed Objective-C format fixture.\n#import <Foundation/Foundation.h>\n\n@interface DXGreeter : NSObject\n- (NSString *)greet:(NSString *)name;\n@end\n\n@implementation DXGreeter\n- (NSString *)greet:(NSString *)name {\n  return [name stringByAppendingString:@\"!\"];\n}\n@end\n",
        },
    ),
    matrix_case(
        name = "matrix_objc_format_fail",
        capability = "format",
        tool = "clang_format",
        binary = "//core/quality/testdata:llvm_clang_format",
        source_class = "objc",
        generated = {
            "matrix/clang_format_dirty.m": "// Seed Objective-C format fixture.\n#import <Foundation/Foundation.h>\n\n@interface DXGreeter : NSObject\n- (NSString *)greet:(NSString *)name;\n@end\n\n@implementation DXGreeter\n- (NSString *)greet:(NSString *)name {return [name stringByAppendingString:@\"!\"];}\n@end\n",
        },
    ),
    matrix_case(
        name = "matrix_objcpp_format_pass",
        capability = "format",
        tool = "clang_format",
        binary = "//core/quality/testdata:llvm_clang_format",
        source_class = "objcpp",
        generated = {
            "matrix/clang_format_clean.mm": "// Seed Objective-C++ format fixture.\n#import <Foundation/Foundation.h>\n\n@interface DXGreeter : NSObject\n- (NSString *)greet:(NSString *)name;\n@end\n\n@implementation DXGreeter\n- (NSString *)greet:(NSString *)name {\n  return [name stringByAppendingString:@\"!\"];\n}\n@end\n\nint DXCount(const char *name) { return name ? 1 : 0; }\n",
        },
    ),
    matrix_case(
        name = "matrix_objcpp_format_fail",
        capability = "format",
        tool = "clang_format",
        binary = "//core/quality/testdata:llvm_clang_format",
        source_class = "objcpp",
        generated = {
            "matrix/clang_format_dirty.mm": "// Seed Objective-C++ format fixture.\n#import <Foundation/Foundation.h>\n\n@interface DXGreeter : NSObject\n- (NSString *)greet:(NSString *)name;\n@end\n\n@implementation DXGreeter\n- (NSString *)greet:(NSString *)name {return [name stringByAppendingString:@\"!\"];}\n@end\n\nint DXCount( const char*name){return name?1:0;}\n",
        },
    ),
    matrix_case(
        name = "matrix_objc_format_config",
        capability = "format",
        tool = "clang_format",
        binary = "//core/quality/testdata:llvm_clang_format",
        source_class = "objc",
        generated = {
            "matrix/clang_format_config.m": "// Seed Objective-C format fixture.\n#import <Foundation/Foundation.h>\n\n@interface DXGreeter : NSObject\n- (NSString *)greet:(NSString *)name;\n@end\n\n@implementation DXGreeter\n- (NSString *)greet:(NSString *)name {\n  return [name stringByAppendingString:@\"!\"];\n}\n@end\n",
        },
        config = ":clang_format_objc_cfg/.clang-format",
        platforms = ["linux_x86_64"],
    ),
    matrix_case(
        name = "matrix_objc_format_config_unsupported",
        capability = "format",
        tool = "clang_format",
        binary = "//core/quality/testdata:llvm_clang_format",
        source_class = "objc",
        generated = {
            "matrix/clang_format_unsupported.m": "// Seed Objective-C format fixture.\n#import <Foundation/Foundation.h>\n\n@interface DXGreeter : NSObject\n- (NSString *)greet:(NSString *)name;\n@end\n\n@implementation DXGreeter\n- (NSString *)greet:(NSString *)name {\n  return [name stringByAppendingString:@\"!\"];\n}\n@end\n",
        },
        config = ":clang_format_cpp_only_cfg/.clang-format",
        platforms = ["linux_x86_64"],
        runner_failure = True,
    ),
    matrix_case(
        name = "matrix_go_format_pass",
        capability = "format",
        tool = "gofumpt",
        binary = "@cc_mvdan_gofumpt//:gofumpt",
        source_class = "go",
        generated = {
            "matrix/gofumpt_clean.go": "// Seed Go format fixture.\npackage gofumpt\n\nconst mode = 0o644\n\nfunc greet(name string) string {\n\treturn \"hello \" + name\n}\n",
        },
        edition_tools = ["gofumpt"],
        edition_values = ["go1.26"],
    ),
    matrix_case(
        name = "matrix_go_format_fail",
        capability = "format",
        tool = "gofumpt",
        binary = "@cc_mvdan_gofumpt//:gofumpt",
        source_class = "go",
        generated = {
            "matrix/gofumpt_dirty.go": "// Seed Go format fixture.\npackage gofumpt\n\nconst mode = 0644\n\nfunc greet( name string)string{return \"hello \"+name}\n",
        },
        edition_tools = ["gofumpt"],
        edition_values = ["go1.26"],
    ),
    matrix_case(
        name = "matrix_go_format_syntax_fail",
        capability = "format",
        tool = "gofumpt",
        binary = "@cc_mvdan_gofumpt//:gofumpt",
        source_class = "go",
        generated = {
            "matrix/gofumpt_syntax.go": "// Seed Go format fixture.\npackage gofumpt\n\nfunc greet(name string) string { BADFMT return \"hello \" + name }\n",
        },
        edition_tools = ["gofumpt"],
        edition_values = ["go1.26"],
        runner_failure = True,
    ),
]
