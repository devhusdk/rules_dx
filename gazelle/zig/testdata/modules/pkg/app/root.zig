// @import("commented_out")
const c = @import("c");
const greet = @import("greet");
const text = "@import(\"in_a_string\")";

pub fn run(buf: []u8) ![]const u8 {
    return greet.greeting(buf, text);
}
