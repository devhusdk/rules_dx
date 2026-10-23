const std = @import("std");
const schema = @import("schema");

pub const version: u32 = schema.version;
pub const max_value: u32 = 1_000_000;

pub const Item = struct {
    name: []const u8,
    quantity: u32,
    price: u32,

    pub fn cost(item: Item) u64 {
        return @as(u64, item.quantity) * item.price;
    }
};

pub const ParseError = error{
    Empty,
    MissingField,
    ExtraField,
    BadNumber,
    OutOfRange,
    DuplicateName,
    OutOfMemory,
};

const packaged_text = @embedFile("items.tsv");

pub fn parse(gpa: std.mem.Allocator, text: []const u8) ParseError![]Item {
    var items: std.ArrayList(Item) = .empty;
    errdefer items.deinit(gpa);
    var lines = std.mem.splitScalar(u8, text, '\n');
    while (lines.next()) |raw| {
        const line = std.mem.trimEnd(u8, raw, "\r");
        if (line.len == 0 or line[0] == '#') continue;
        var fields = std.mem.splitScalar(u8, line, '\t');
        const name = fields.next() orelse return error.MissingField;
        const quantity = try number(fields.next() orelse return error.MissingField);
        const price = try number(fields.next() orelse return error.MissingField);
        if (fields.next() != null) return error.ExtraField;
        if (name.len == 0) return error.MissingField;
        if (find(items.items, name) != null) return error.DuplicateName;
        try items.append(gpa, .{ .name = name, .quantity = quantity, .price = price });
    }
    if (items.items.len == 0) return error.Empty;
    return items.toOwnedSlice(gpa);
}

pub fn packaged(gpa: std.mem.Allocator) ParseError![]Item {
    return parse(gpa, packaged_text);
}

pub fn find(items: []const Item, name: []const u8) ?Item {
    for (items) |item| {
        if (std.mem.eql(u8, item.name, name)) return item;
    }
    return null;
}

pub fn total(items: []const Item) u64 {
    var sum: u64 = 0;
    for (items) |item| sum += item.cost();
    return sum;
}

fn number(field: []const u8) ParseError!u32 {
    const value = std.fmt.parseInt(u32, field, 10) catch |err| switch (err) {
        error.Overflow => return error.OutOfRange,
        error.InvalidCharacter => return error.BadNumber,
    };
    if (value > max_value) return error.OutOfRange;
    return value;
}
