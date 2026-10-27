// Dumps the namespace lists that gazelle treats as the .NET platform.
//   dump            print both lists to stdout
//   dump --write    rewrite the checked-in lists under BUILD_WORKSPACE_DIRECTORY
//   dump --check    compare against the checked-in lists (runfiles), exit 1 on drift
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Reflection.Metadata;
using System.Reflection.PortableExecutable;
using System.Runtime.InteropServices;

public static class DotnetPlatformDump
{
    private const string FrameworkFile = "dotnet_namespaces.txt";
    private const string FSharpFile = "fsharp_core_namespaces.txt";
    private const string Dir = "gazelle/common";

    private static void AddWithAncestors(SortedSet<string> set, string ns)
    {
        if (ns.Length == 0)
        {
            return;
        }
        var parts = ns.Split('.');
        for (var i = 1; i <= parts.Length; i++)
        {
            set.Add(string.Join(".", parts.Take(i)));
        }
    }

    private static SortedSet<string> Framework()
    {
        var set = new SortedSet<string>(StringComparer.Ordinal);
        foreach (var path in Directory.GetFiles(RuntimeEnvironment.GetRuntimeDirectory(), "*.dll"))
        {
            var name = Path.GetFileName(path);
            if (name.StartsWith("System.Private.", StringComparison.Ordinal))
            {
                continue;
            }
            try
            {
                using var stream = File.OpenRead(path);
                using var pe = new PEReader(stream);
                if (!pe.HasMetadata)
                {
                    continue;
                }
                var md = pe.GetMetadataReader();
                foreach (var h in md.TypeDefinitions)
                {
                    var t = md.GetTypeDefinition(h);
                    if (!t.GetDeclaringType().IsNil || (t.Attributes & TypeAttributes.VisibilityMask) != TypeAttributes.Public)
                    {
                        continue;
                    }
                    AddWithAncestors(set, md.GetString(t.Namespace));
                }
                foreach (var h in md.ExportedTypes)
                {
                    var t = md.GetExportedType(h);
                    if (t.Implementation.Kind != HandleKind.AssemblyReference)
                    {
                        continue;
                    }
                    AddWithAncestors(set, md.GetString(t.Namespace));
                }
            }
            catch (BadImageFormatException)
            {
            }
        }
        return set;
    }

    private static bool HasAttr(Type t, string name, int mask)
    {
        foreach (var a in t.GetCustomAttributesData())
        {
            if (a.AttributeType.Name == name && a.ConstructorArguments.Count > 0 && a.ConstructorArguments[0].Value is int flags && (flags & mask) == mask)
            {
                return true;
            }
        }
        return false;
    }

    // SourceConstructFlags.Module = 7, CompilationRepresentationFlags.ModuleSuffix = 4.
    private static bool IsModule(Type t)
    {
        return HasAttr(t, "CompilationMappingAttribute", 7) && (((int)Flags(t, "CompilationMappingAttribute")) & 31) == 7;
    }

    private static int Flags(Type t, string name)
    {
        foreach (var a in t.GetCustomAttributesData())
        {
            if (a.AttributeType.Name == name && a.ConstructorArguments.Count > 0 && a.ConstructorArguments[0].Value is int flags)
            {
                return flags;
            }
        }
        return 0;
    }

    private static string SourceName(Type t)
    {
        var name = t.Name;
        if (HasAttr(t, "CompilationRepresentationAttribute", 4) && name.EndsWith("Module", StringComparison.Ordinal))
        {
            name = name.Substring(0, name.Length - "Module".Length);
        }
        return name;
    }

    private static void WalkModules(Type t, string prefix, ISet<string> modules)
    {
        foreach (var n in t.GetNestedTypes(BindingFlags.Public))
        {
            if (IsModule(n))
            {
                var full = prefix + "." + SourceName(n);
                modules.Add(full);
                WalkModules(n, full, modules);
            }
        }
    }

    private static SortedSet<string> FSharpCore()
    {
        var asm = typeof(Microsoft.FSharp.Core.Unit).Assembly;
        var set = new SortedSet<string>(StringComparer.Ordinal);
        foreach (var t in asm.GetExportedTypes())
        {
            if (t.IsNested)
            {
                continue;
            }
            var ns = t.Namespace ?? "";
            AddWithAncestors(set, ns);
            if (IsModule(t))
            {
                var full = ns + "." + SourceName(t);
                set.Add(full);
                WalkModules(t, full, set);
            }
        }
        var all = new SortedSet<string>(StringComparer.Ordinal);
        foreach (var n in set)
        {
            all.Add(n);
            if (n.StartsWith("Microsoft.FSharp", StringComparison.Ordinal))
            {
                all.Add(n.Substring("Microsoft.".Length));
            }
        }
        return all;
    }

    private static string Render(string title, SortedSet<string> names)
    {
        return "# " + title + "\n# Generated by //gazelle/dotnetplatform:dump; do not edit.\n"
            + "# Regenerate: bazel run //gazelle/dotnetplatform:dump -- --write\n"
            + "# Read from the Linux shared framework; other OS builds may differ slightly.\n"
            + string.Join("\n", names) + "\n";
    }

    private static string Read(string path)
    {
        return File.Exists(path) ? File.ReadAllText(path) : "";
    }

    public static int Main(string[] args)
    {
        var fw = Render("Public namespaces of Microsoft.NETCore.App (net10.0).", Framework());
        var fs = Render("Namespaces and modules of FSharp.Core (10.1.201).", FSharpCore());
        var mode = args.Length > 0 ? args[0] : "";
        if (mode == "--write")
        {
            var root = Environment.GetEnvironmentVariable("BUILD_WORKSPACE_DIRECTORY");
            if (string.IsNullOrEmpty(root))
            {
                Console.Error.WriteLine("BUILD_WORKSPACE_DIRECTORY is not set; use bazel run");
                return 1;
            }
            File.WriteAllText(Path.Combine(root, Dir, FrameworkFile), fw);
            File.WriteAllText(Path.Combine(root, Dir, FSharpFile), fs);
            return 0;
        }
        if (mode == "--check")
        {
            var src = Environment.GetEnvironmentVariable("TEST_SRCDIR") ?? ".";
            var ws = Environment.GetEnvironmentVariable("TEST_WORKSPACE") ?? "_main";
            var bad = 0;
            foreach (var (file, want) in new[] { (FrameworkFile, fw), (FSharpFile, fs) })
            {
                var got = Read(Path.Combine(src, ws, Dir, file));
                if (got != want)
                {
                    Console.Error.WriteLine("DRIFT: " + file + " is stale; run: bazel run //gazelle/dotnetplatform:dump -- --write");
                    bad = 1;
                }
            }
            return bad;
        }
        Console.Write(fw);
        Console.Write(fs);
        return 0;
    }
}
