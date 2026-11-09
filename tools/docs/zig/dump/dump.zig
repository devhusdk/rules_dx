//! Reads Zig sources with the compiler's own parser (`std.zig.Ast`) and writes the doc comments
//! and declarations of each file as JSON, schema 1, for the DocIr extractor (`zig_extract`).
//!
//! Usage: zig_doc_dump --out FILE SOURCE...
//!
//! Every `std.zig.Ast` call lives in this file, so a Zig bump needs fixes only here.

const std = @import("std");
const Ast = std.zig.Ast;
const Allocator = std.mem.Allocator;

const schema_version = 1;

/// `Ast.parse` takes a bare mode up to Zig 0.16 and an options struct from 0.17.
const parse_options = if (@hasDecl(Ast, "ParseOptions")) Ast.ParseOptions{ .mode = .zig } else Ast.Mode.zig;

const Param = struct {
    name: []const u8,
    type: []const u8,
};

const Field = struct {
    name: []const u8,
    type: []const u8,
    doc: []const u8,
    line: usize,
};

const Decl = struct {
    name: []const u8,
    public: bool,
    /// fn, struct, enum, union, opaque, error, const, var or import.
    kind: []const u8,
    doc: []const u8,
    line: usize,
    signature: []const u8,
    params: []const Param = &.{},
    returns: []const u8 = "",
    fields: []const Field = &.{},
    members: []const Decl = &.{},
    /// The `@import` target of `const X = @import("target.zig")`.
    import: []const u8 = "",
};

const File = struct {
    path: []const u8,
    doc: []const u8,
    imports: []const []const u8,
    fields: []const Field,
    decls: []const Decl,
};

const Dump = struct {
    schema: u32 = schema_version,
    files: []const File,
};

const usage = "usage: zig_doc_dump --out FILE SOURCE...\n";

const Walker = struct {
    arena: Allocator,
    tree: Ast,

    fn line(w: *const Walker, token: Ast.TokenIndex) usize {
        return w.tree.tokenLocation(0, token).line + 1;
    }

    /// The source text from the start of `first` to the end of `last`.
    fn span(w: *const Walker, first: Ast.TokenIndex, last: Ast.TokenIndex) []const u8 {
        const start = w.tree.tokenStart(first);
        const end = w.tree.tokenStart(last) + w.tree.tokenSlice(last).len;
        return w.tree.source[start..end];
    }

    /// The text of the `///` or `//!` comment lines that run up to `first`.
    fn docBefore(w: *const Walker, first: Ast.TokenIndex, tag: std.zig.Token.Tag) ![]const u8 {
        var start = first;
        while (start > 0 and w.tree.tokenTag(start - 1) == tag) start -= 1;
        return w.docLines(start, first);
    }

    fn docLines(w: *const Walker, start: Ast.TokenIndex, end: Ast.TokenIndex) ![]const u8 {
        var out: std.ArrayList(u8) = .empty;
        var index = start;
        while (index < end) : (index += 1) {
            const raw = std.mem.trimEnd(u8, w.tree.tokenSlice(index)[3..], "\r\n");
            const text = if (std.mem.startsWith(u8, raw, " ")) raw[1..] else raw;
            if (index > start) try out.append(w.arena, '\n');
            try out.appendSlice(w.arena, text);
        }
        return out.items;
    }

    fn fileDoc(w: *const Walker) ![]const u8 {
        var end: Ast.TokenIndex = 0;
        while (w.tree.tokenTag(end) == .container_doc_comment) end += 1;
        return w.docLines(0, end);
    }

    fn imports(w: *const Walker) ![]const []const u8 {
        var out: std.ArrayList([]const u8) = .empty;
        const count: Ast.TokenIndex = @intCast(w.tree.tokens.len);
        var index: Ast.TokenIndex = 0;
        while (index + 2 < count) : (index += 1) {
            if (w.tree.tokenTag(index) != .builtin) continue;
            if (!std.mem.eql(u8, w.tree.tokenSlice(index), "@import")) continue;
            if (w.tree.tokenTag(index + 1) != .l_paren) continue;
            if (w.tree.tokenTag(index + 2) != .string_literal) continue;
            const text = w.tree.tokenSlice(index + 2);
            try out.append(w.arena, text[1 .. text.len - 1]);
        }
        return out.items;
    }

    fn field(w: *const Walker, node: Ast.Node.Index) !?Field {
        const member = w.tree.fullContainerField(node) orelse return null;
        const type_text = if (member.ast.type_expr.unwrap()) |t| w.tree.getNodeSource(t) else "";
        return .{
            .name = if (member.ast.tuple_like) "" else w.tree.tokenSlice(member.ast.main_token),
            .type = type_text,
            .doc = try w.docBefore(member.firstToken(), .doc_comment),
            .line = w.line(member.ast.main_token),
        };
    }

    fn fields(w: *const Walker, nodes: []const Ast.Node.Index) ![]const Field {
        var out: std.ArrayList(Field) = .empty;
        for (nodes) |node| {
            if (try w.field(node)) |item| try out.append(w.arena, item);
        }
        return out.items;
    }

    fn decls(w: *const Walker, nodes: []const Ast.Node.Index) Allocator.Error![]const Decl {
        var out: std.ArrayList(Decl) = .empty;
        for (nodes) |node| {
            if (try w.decl(node)) |item| try out.append(w.arena, item);
        }
        return out.items;
    }

    /// One declaration node, or null for fields, tests, comptime blocks and anonymous protos.
    fn decl(w: *const Walker, node: Ast.Node.Index) Allocator.Error!?Decl {
        var buffer: [1]Ast.Node.Index = undefined;
        if (w.tree.fullFnProto(&buffer, node)) |proto| return try w.fnDecl(proto);
        if (w.tree.fullVarDecl(node)) |variable| return try w.varDecl(variable);
        return null;
    }

    fn fnDecl(w: *const Walker, proto: Ast.full.FnProto) Allocator.Error!?Decl {
        const name_token = proto.name_token orelse return null;
        const first = proto.firstToken();
        const params = try w.paramList(proto);
        var returns: []const u8 = "";
        if (proto.ast.return_type.unwrap()) |return_type| {
            const first_return = w.tree.firstToken(return_type);
            const from = if (first_return > 0 and w.tree.tokenTag(first_return - 1) == .bang) first_return - 1 else first_return;
            returns = w.span(from, w.tree.lastToken(return_type));
        }
        return .{
            .name = w.tree.tokenSlice(name_token),
            .public = proto.visib_token != null,
            .kind = "fn",
            .doc = try w.docBefore(first, .doc_comment),
            .line = w.line(name_token),
            .signature = w.span(first, w.tree.lastToken(proto.ast.proto_node)),
            .params = params,
            .returns = returns,
        };
    }

    fn paramList(w: *const Walker, proto: Ast.full.FnProto) ![]const Param {
        var out: std.ArrayList(Param) = .empty;
        var iterator = proto.iterate(&w.tree);
        while (iterator.next()) |param| {
            var type_text: []const u8 = "";
            if (param.type_expr) |type_node| {
                type_text = w.tree.getNodeSource(type_node);
            } else if (param.anytype_ellipsis3) |token| {
                type_text = w.tree.tokenSlice(token);
            }
            if (param.comptime_noalias) |token| {
                type_text = try std.fmt.allocPrint(w.arena, "{s} {s}", .{ w.tree.tokenSlice(token), type_text });
            }
            try out.append(w.arena, .{
                .name = if (param.name_token) |token| w.tree.tokenSlice(token) else "",
                .type = type_text,
            });
        }
        return out.items;
    }

    fn varDecl(w: *const Walker, variable: Ast.full.VarDecl) Allocator.Error!Decl {
        const mut_token = variable.ast.mut_token;
        const name_token = mut_token + 1;
        const first = variable.firstToken();
        var result: Decl = .{
            .name = w.tree.tokenSlice(name_token),
            .public = variable.visib_token != null,
            .kind = if (w.tree.tokenTag(mut_token) == .keyword_const) "const" else "var",
            .doc = try w.docBefore(first, .doc_comment),
            .line = w.line(name_token),
            .signature = "",
        };
        const init_node = variable.ast.init_node.unwrap();
        var head_last = name_token;
        if (variable.ast.type_node.unwrap()) |type_node| head_last = w.tree.lastToken(type_node);
        result.signature = w.span(first, head_last);
        const value = init_node orelse return result;

        var container_buffer: [2]Ast.Node.Index = undefined;
        if (w.tree.fullContainerDecl(&container_buffer, value)) |container| {
            const keyword = w.tree.tokenTag(container.ast.main_token);
            result.kind = switch (keyword) {
                .keyword_struct => "struct",
                .keyword_enum => "enum",
                .keyword_union => "union",
                .keyword_opaque => "opaque",
                else => result.kind,
            };
            var brace = container.ast.main_token;
            while (w.tree.tokenTag(brace) != .l_brace) brace += 1;
            result.signature = std.mem.trimEnd(u8, w.span(first, brace - 1), " \t\r\n");
            result.fields = try w.fields(container.ast.members);
            result.members = try w.decls(container.ast.members);
            return result;
        }
        switch (w.tree.nodeTag(value)) {
            .error_set_decl => {
                result.kind = "error";
                result.signature = w.span(first, w.tree.lastToken(value));
            },
            .builtin_call_two, .builtin_call_two_comma => {
                var call_buffer: [2]Ast.Node.Index = undefined;
                const call_params = w.tree.builtinCallParams(&call_buffer, value) orelse return result;
                const is_import = std.mem.eql(u8, w.tree.tokenSlice(w.tree.nodeMainToken(value)), "@import");
                if (is_import and call_params.len == 1 and w.tree.nodeTag(call_params[0]) == .string_literal) {
                    const text = w.tree.tokenSlice(w.tree.nodeMainToken(call_params[0]));
                    result.kind = "import";
                    result.import = text[1 .. text.len - 1];
                    result.signature = w.span(first, w.tree.lastToken(value));
                }
            },
            else => {
                const init_text = w.tree.getNodeSource(value);
                const short = init_text.len <= 80 and std.mem.indexOfScalar(u8, init_text, '\n') == null;
                if (short) {
                    result.signature = try std.fmt.allocPrint(w.arena, "{s} = {s}", .{ result.signature, init_text });
                }
            },
        }
        return result;
    }
};

/// Parses one source file into its dump record. The source must outlive `arena`'s use.
fn dumpSource(arena: Allocator, path: []const u8, source: [:0]const u8) !File {
    var walker: Walker = .{ .arena = arena, .tree = try Ast.parse(arena, source, parse_options) };
    if (walker.tree.errors.len > 0) {
        std.debug.print("zig_doc_dump: {s}: syntax error\n", .{path});
        return error.SyntaxError;
    }
    const root = walker.tree.rootDecls();
    return .{
        .path = path,
        .doc = try walker.fileDoc(),
        .imports = try walker.imports(),
        .fields = try walker.fields(root),
        .decls = try walker.decls(root),
    };
}

pub fn main(init: std.process.Init) !void {
    const arena = init.arena.allocator();
    const args = try init.minimal.args.toSlice(arena);
    if (args.len < 4 or !std.mem.eql(u8, args[1], "--out")) {
        std.debug.print("{s}", .{usage});
        std.process.exit(2);
    }
    var files: std.ArrayList(File) = .empty;
    for (args[3..]) |path| {
        const source = try std.Io.Dir.cwd().readFileAllocOptions(init.io, path, arena, .limited(64 << 20), .of(u8), 0);
        try files.append(arena, try dumpSource(arena, path, source));
    }
    const dump: Dump = .{ .files = files.items };
    const json = try std.json.Stringify.valueAlloc(arena, dump, .{ .whitespace = .indent_1 });
    try std.Io.Dir.cwd().writeFile(init.io, .{ .sub_path = args[2], .data = json });
}

test "dump keeps docs, signatures and nested members and skips tests" {
    var arena_state: std.heap.ArenaAllocator = .init(std.testing.allocator);
    defer arena_state.deinit();
    const arena = arena_state.allocator();
    const source =
        \\//! File doc.
        \\const std = @import("std");
        \\pub const util = @import("util.zig");
        \\/// A shape.
        \\pub const Shape = struct {
        \\    /// Width.
        \\    w: f64,
        \\    pub fn area(self: Shape) f64 { return self.w; }
        \\};
        \\/// Adds.
        \\pub fn add(comptime T: type, a: T, b: anytype) !T { return a + b; }
        \\fn hidden() void {}
        \\test "x" {}
        \\
    ;
    const file = try dumpSource(arena, "a.zig", source);
    try std.testing.expectEqualStrings("File doc.", file.doc);
    try std.testing.expectEqual(@as(usize, 2), file.imports.len);
    try std.testing.expectEqual(@as(usize, 5), file.decls.len);
    try std.testing.expectEqualStrings("import", file.decls[1].kind);
    try std.testing.expectEqualStrings("util.zig", file.decls[1].import);
    const shape = file.decls[2];
    try std.testing.expectEqualStrings("pub const Shape = struct", shape.signature);
    try std.testing.expectEqualStrings("A shape.", shape.doc);
    try std.testing.expectEqualStrings("Width.", shape.fields[0].doc);
    try std.testing.expectEqualStrings("area", shape.members[0].name);
    const add = file.decls[3];
    try std.testing.expectEqualStrings("pub fn add(comptime T: type, a: T, b: anytype) !T", add.signature);
    try std.testing.expectEqualStrings("!T", add.returns);
    try std.testing.expectEqualStrings("comptime type", add.params[0].type);
    try std.testing.expectEqualStrings("anytype", add.params[2].type);
    try std.testing.expect(!file.decls[4].public);
}
