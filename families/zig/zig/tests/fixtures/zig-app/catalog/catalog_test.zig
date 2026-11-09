const std = @import("std");
const catalog = @import("catalog");

const testing = std.testing;

test "packaged catalog parses in order" {
    const items = try catalog.packaged(testing.allocator);
    defer testing.allocator.free(items);
    try testing.expectEqual(@as(usize, 3), items.len);
    try testing.expectEqualStrings("lamp", items[0].name);
    try testing.expectEqual(@as(u32, 12), items[1].quantity);
    try testing.expectEqual(@as(u64, 7388), catalog.total(items));
}

test "schema comes from the generated module" {
    try testing.expectEqual(@as(u32, 2), catalog.version);
}

test "find returns the named item" {
    const items = try catalog.packaged(testing.allocator);
    defer testing.allocator.free(items);
    try testing.expectEqual(@as(u32, 199), catalog.find(items, "cable").?.price);
    try testing.expect(catalog.find(items, "bulb") == null);
}

test "comments, blank lines and CRLF are skipped" {
    const items = try catalog.parse(testing.allocator, "# head\r\n\r\nbulb\t2\t7\r\n");
    defer testing.allocator.free(items);
    try testing.expectEqual(@as(u64, 14), catalog.total(items));
}

test "malformed lines are rejected" {
    const gpa = testing.allocator;
    try testing.expectError(error.Empty, catalog.parse(gpa, "# only a comment\n"));
    try testing.expectError(error.MissingField, catalog.parse(gpa, "bulb\t2\n"));
    try testing.expectError(error.MissingField, catalog.parse(gpa, "\t2\t7\n"));
    try testing.expectError(error.ExtraField, catalog.parse(gpa, "bulb\t2\t7\t9\n"));
    try testing.expectError(error.BadNumber, catalog.parse(gpa, "bulb\ttwo\t7\n"));
    try testing.expectError(error.BadNumber, catalog.parse(gpa, "bulb\t1.5\t7\n"));
    try testing.expectError(error.OutOfRange, catalog.parse(gpa, "bulb\t1000001\t7\n"));
    try testing.expectError(error.OutOfRange, catalog.parse(gpa, "bulb\t2\t99999999999\n"));
    try testing.expectError(error.DuplicateName, catalog.parse(gpa, "bulb\t2\t7\nbulb\t1\t1\n"));
}
