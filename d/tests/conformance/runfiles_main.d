module runfiles_main;

import std.algorithm.searching : startsWith;
import std.file : readText;
import std.process : environment;
import std.stdio : File, write;
import std.string : chomp;

string resolve(string rpath)
{
    auto dir = environment.get("RUNFILES_DIR");
    if (dir.length > 0)
        return dir ~ "/" ~ rpath;
    auto manifest = environment.get("RUNFILES_MANIFEST_FILE");
    if (manifest.length == 0)
        throw new Exception("no runfiles");
    foreach (line; File(manifest).byLineCopy)
    {
        auto entry = line.chomp("\r");
        if (entry.startsWith(rpath ~ " "))
            return entry[rpath.length + 1 .. $];
    }
    throw new Exception("runfile not in manifest: " ~ rpath);
}

void main(string[] args)
{
    write(readText(resolve(args[1])));
}
