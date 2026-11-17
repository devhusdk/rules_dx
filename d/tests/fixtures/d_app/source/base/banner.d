/// The generated banner text.
module base.banner;

/// The first line of the generated banner file.
enum banner = import("banner.txt");

unittest
{
    assert(banner == "d-app 1.2.3\n");
}
