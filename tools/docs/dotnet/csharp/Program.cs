using System;
using System.Collections;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Linq;
using System.Net;
using System.Text;
using System.Text.Encodings.Web;
using System.Text.Json;
using System.Text.RegularExpressions;
using Docfx.Common;
using Docfx.Dotnet;
using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;

namespace Dx.Docs.CsharpApi;

public static class Program
{
    private const string Schema = "dx.dotnet-api/1";

    private sealed class Context
    {
        public string Assembly = "";
        public List<string> Srcs = new List<string>();
        public List<string> Defines = new List<string>();
        public bool AllowUnsafe;
        public List<string> References = new List<string>();
        public List<string> FrameworkRefs = new List<string>();
        public string OwnRef = "";
    }

    private sealed class Failure : Exception
    {
        public Failure(string message) : base(message)
        {
        }
    }

    public static int Main(string[] args)
    {
        string contextPath = "";
        string outPath = "";
        string scratch = "";
        for (int i = 0; i + 1 < args.Length; i += 2)
        {
            switch (args[i])
            {
                case "--context":
                    contextPath = args[i + 1];
                    break;
                case "--out":
                    outPath = args[i + 1];
                    break;
                case "--scratch":
                    scratch = args[i + 1];
                    break;
                default:
                    Console.Error.WriteLine("csharp_api: unknown argument " + args[i]);
                    return 2;
            }
        }
        if (contextPath == "" || outPath == "" || scratch == "")
        {
            Console.Error.WriteLine("usage: csharp_api --context FILE --scratch DIR --out FILE");
            return 2;
        }
        try
        {
            string json = Run(ReadContext(contextPath), scratch);
            File.WriteAllText(outPath, json, new UTF8Encoding(false));
            return 0;
        }
        catch (Failure failure)
        {
            Console.Error.WriteLine("csharp_api: " + failure.Message);
            return 1;
        }
        catch (Exception error)
        {
            Console.Error.WriteLine("csharp_api: " + error);
            return 1;
        }
    }

    private static Context ReadContext(string path)
    {
        using JsonDocument document = JsonDocument.Parse(File.ReadAllText(path));
        JsonElement root = document.RootElement;
        Context context = new Context();
        context.Assembly = root.GetProperty("assembly").GetString();
        context.Srcs = Strings(root, "srcs");
        context.Defines = Strings(root, "defines");
        context.AllowUnsafe = root.GetProperty("allow_unsafe_blocks").GetBoolean();
        context.References = Strings(root, "references");
        context.FrameworkRefs = Strings(root, "framework_refs");
        context.OwnRef = root.GetProperty("own_ref").GetString();
        return context;
    }

    private static List<string> Strings(JsonElement root, string name)
    {
        List<string> values = new List<string>();
        foreach (JsonElement element in root.GetProperty(name).EnumerateArray())
        {
            values.Add(element.GetString());
        }
        return values;
    }

    private static string Run(Context context, string scratch)
    {
        if (context.Srcs.Count == 0)
        {
            throw new Failure("the library has no C# sources");
        }
        string root = Path.GetFullPath(scratch);
        string work = Path.Combine(root, "work");
        string api = Path.Combine(root, "api");
        Directory.CreateDirectory(work);
        Directory.CreateDirectory(api);
        string fakeRoot = Path.Combine(Path.GetTempPath(), "dx-csharp-api-" + Environment.ProcessId.ToString(CultureInfo.InvariantCulture));
        try
        {
            FakeSdk(fakeRoot, context.FrameworkRefs);
            Environment.SetEnvironmentVariable("DOTNET_ROOT", fakeRoot);
            Environment.SetEnvironmentVariable("PATH", fakeRoot + Path.PathSeparator + Environment.GetEnvironmentVariable("PATH"));
            string cwd = Directory.GetCurrentDirectory();
            string configPath = Path.Combine(work, "docfx.json");
            File.WriteAllText(configPath, DocfxConfig(context, cwd, api), new UTF8Encoding(false));
            Logger.ResetCount();
            DotnetApiCatalog.GenerateManagedReferenceYamlFiles(configPath).GetAwaiter().GetResult();
            if (Logger.ErrorCount > 0 || Logger.WarningCount > 0)
            {
                throw new Failure("docfx reported " + Logger.ErrorCount + " error(s) and " + Logger.WarningCount + " warning(s)");
            }
            List<Entry> items = ReadItems(api, context);
            CrossCheck(context, items);
            Locate(context, items);
            return Render(context, items);
        }
        finally
        {
            try
            {
                if (Directory.Exists(fakeRoot))
                {
                    Directory.Delete(fakeRoot, true);
                }
            }
            catch (IOException)
            {
            }
        }
    }

    private static void FakeSdk(string fakeRoot, List<string> frameworkRefs)
    {
        if (frameworkRefs.Count == 0)
        {
            throw new Failure("the library has no framework reference assemblies");
        }
        Directory.CreateDirectory(fakeRoot);
        string host = Path.Combine(fakeRoot, OperatingSystem.IsWindows() ? "dotnet.exe" : "dotnet");
        File.WriteAllBytes(host, new byte[0]);
        if (!OperatingSystem.IsWindows())
        {
            File.SetUnixFileMode(host, UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute | UnixFileMode.GroupRead | UnixFileMode.GroupExecute | UnixFileMode.OtherRead | UnixFileMode.OtherExecute);
        }
        string runtime = Environment.Version.ToString(2);
        string refDirectory = Path.Combine(fakeRoot, "packs", "Microsoft.NETCore.App.Ref", runtime + ".0", "ref", "net" + runtime);
        Directory.CreateDirectory(refDirectory);
        foreach (string reference in frameworkRefs)
        {
            string target = Path.Combine(refDirectory, Path.GetFileName(reference));
            if (File.Exists(target))
            {
                continue;
            }
            File.Copy(Path.GetFullPath(reference), target);
        }
    }

    private static string DocfxConfig(Context context, string cwd, string api)
    {
        using MemoryStream stream = new MemoryStream();
        using (Utf8JsonWriter writer = new Utf8JsonWriter(stream))
        {
            writer.WriteStartObject();
            writer.WriteStartArray("metadata");
            writer.WriteStartObject();
            WriteMapping(writer, "src", cwd, context.Srcs);
            if (context.References.Count > 0)
            {
                WriteMapping(writer, "references", cwd, context.References);
            }
            writer.WriteString("output", api);
            writer.WriteString("outputFormat", "mref");
            writer.WriteBoolean("disableGitFeatures", true);
            writer.WriteBoolean("noRestore", true);
            writer.WriteString("codeSourceBasePath", cwd);
            writer.WriteStartObject("properties");
            writer.WriteString("DefineConstants", string.Join(";", context.Defines));
            writer.WriteString("AllowUnsafeBlocks", context.AllowUnsafe ? "true" : "false");
            writer.WriteEndObject();
            writer.WriteEndObject();
            writer.WriteEndArray();
            writer.WriteEndObject();
        }
        return Encoding.UTF8.GetString(stream.ToArray());
    }

    private static void WriteMapping(Utf8JsonWriter writer, string name, string cwd, List<string> files)
    {
        writer.WriteStartArray(name);
        writer.WriteStartObject();
        writer.WriteString("src", cwd);
        writer.WriteStartArray("files");
        foreach (string file in files)
        {
            writer.WriteStringValue(file.Replace('\\', '/'));
        }
        writer.WriteEndArray();
        writer.WriteEndObject();
        writer.WriteEndArray();
    }

    private sealed class Entry
    {
        public string DocId = "";
        public string Kind = "";
        public string Name = "";
        public string DisplayName = "";
        public string Parent = "";
        public string Signature = "";
        public string Summary = "";
        public string Remarks = "";
        public List<string[]> Params = new List<string[]>();
        public string[] Returns;
        public List<string> Examples = new List<string>();
        public string File = "";
        public int Line;
        public List<string> Implements = new List<string>();
        public string Uid = "";
        public string ParentUid = "";
        public List<string> ImplementsUids = new List<string>();
    }

    private static object Get(object node, string key)
    {
        IDictionary map = node as IDictionary;
        if (map != null && map.Contains(key))
        {
            return map[key];
        }
        return null;
    }

    private static string Str(object node, string key)
    {
        object value = Get(node, key);
        return value == null ? "" : Convert.ToString(value, CultureInfo.InvariantCulture);
    }

    private static List<object> Seq(object node, string key)
    {
        List<object> values = new List<object>();
        IList list = Get(node, key) as IList;
        if (list != null)
        {
            foreach (object element in list)
            {
                values.Add(element);
            }
        }
        return values;
    }

    private static List<Entry> ReadItems(string api, Context context)
    {
        List<Entry> entries = new List<Entry>();
        List<string> files = Directory.GetFiles(api, "*.yml").OrderBy(f => f, StringComparer.Ordinal).ToList();
        foreach (string file in files)
        {
            if (Path.GetFileName(file) == "toc.yml")
            {
                continue;
            }
            object document = YamlUtility.Deserialize<object>(file);
            foreach (object item in Seq(document, "items"))
            {
                string commentId = Str(item, "commentId");
                if (commentId == "" || commentId.StartsWith("Overload:", StringComparison.Ordinal))
                {
                    continue;
                }
                entries.Add(ReadItem(item, commentId, context));
            }
        }
        Dictionary<string, string> byUid = new Dictionary<string, string>(StringComparer.Ordinal);
        foreach (Entry entry in entries)
        {
            byUid[entry.Uid] = entry.DocId;
        }
        HashSet<string> enumTypes = new HashSet<string>(entries.Where(e => e.Kind == "enum").Select(e => e.DocId), StringComparer.Ordinal);
        foreach (Entry entry in entries)
        {
            string parent;
            if (entry.ParentUid != "" && byUid.TryGetValue(entry.ParentUid, out parent))
            {
                entry.Parent = parent;
                if (entry.Kind == "field" && parent.StartsWith("T:", StringComparison.Ordinal) && enumTypes.Contains(parent))
                {
                    entry.Kind = "enum_field";
                }
            }
            foreach (string uid in entry.ImplementsUids)
            {
                string target;
                if (byUid.TryGetValue(uid, out target))
                {
                    entry.Implements.Add(target);
                }
            }
        }
        entries.Sort((a, b) => string.CompareOrdinal(a.DocId, b.DocId));
        return entries;
    }

    private static Entry ReadItem(object item, string commentId, Context context)
    {
        Entry entry = new Entry();
        entry.DocId = commentId;
        entry.Uid = Str(item, "uid");
        entry.ParentUid = Str(item, "parent");
        string name = Str(item, "name");
        string type = Str(item, "type");
        entry.DisplayName = name;
        entry.Name = ShortName(name, type);
        entry.Kind = Kind(type, name, Str(Get(item, "syntax"), "content"), commentId);
        object syntax = Get(item, "syntax");
        entry.Signature = entry.Kind == "namespace" ? "namespace " + name : Str(syntax, "content").Trim();
        entry.Summary = Clean(Str(item, "summary"));
        entry.Remarks = Clean(Str(item, "remarks"));
        foreach (object example in Seq(item, "example"))
        {
            entry.Examples.Add(Clean(Convert.ToString(example, CultureInfo.InvariantCulture)));
        }
        foreach (object parameter in Seq(syntax, "parameters"))
        {
            entry.Params.Add(new[] { Str(parameter, "id"), TypeText(Str(parameter, "type")), Clean(Str(parameter, "description")) });
        }
        object returns = Get(syntax, "return");
        if (returns != null)
        {
            entry.Returns = new[] { TypeText(Str(returns, "type")), Clean(Str(returns, "description")) };
        }
        foreach (object implemented in Seq(item, "implements"))
        {
            entry.ImplementsUids.Add(Convert.ToString(implemented, CultureInfo.InvariantCulture));
        }
        object source = Get(item, "source");
        string path = Str(source, "path");
        if (path != "")
        {
            entry.File = ResolveSource(path, context, commentId);
            entry.Line = int.Parse(Str(source, "startLine"), CultureInfo.InvariantCulture);
        }
        return entry;
    }

    private static string ShortName(string name, string type)
    {
        if (type == "Namespace")
        {
            return name;
        }
        int paren = name.IndexOf('(');
        if (paren >= 0)
        {
            name = name.Substring(0, paren);
        }
        int bracket = name.IndexOf('[');
        if (bracket >= 0)
        {
            name = name.Substring(0, bracket);
        }
        return name;
    }

    private static string Kind(string type, string name, string content, string commentId)
    {
        switch (type)
        {
            case "Namespace": return "namespace";
            case "Class": return "class";
            case "Struct": return "struct";
            case "Interface": return "interface";
            case "Enum": return "enum";
            case "Delegate": return "delegate";
            case "Constructor": return "constructor";
            case "Method": return "method";
            case "Event": return "event";
            case "Operator": return "operator";
            case "Property": return name.Contains('[') ? "indexer" : "property";
            case "Field": return Regex.IsMatch(content, @"\bconst\b") ? "const" : "field";
            default: throw new Failure("'" + commentId + "' has the unsupported docfx type '" + type + "'");
        }
    }

    private static string ResolveSource(string path, Context context, string commentId)
    {
        string normal = path.Replace('\\', '/');
        string full = Path.GetFullPath(path.Replace('\\', Path.DirectorySeparatorChar));
        foreach (string src in context.Srcs)
        {
            string exec = src.Replace('\\', '/');
            if (normal == exec || normal.EndsWith("/" + exec, StringComparison.Ordinal) || full == Path.GetFullPath(src))
            {
                return exec;
            }
        }
        throw new Failure("'" + commentId + "' is declared in '" + path + "', which is not a declared source");
    }

    private static string TypeText(string uid)
    {
        return WebUtility.UrlDecode(uid);
    }

    private static string Display(string uid)
    {
        string text = WebUtility.UrlDecode(uid);
        int paren = text.IndexOf('(');
        if (paren >= 0)
        {
            text = text.Substring(0, paren);
        }
        int dot = text.LastIndexOf('.');
        return dot >= 0 ? text.Substring(dot + 1) : text;
    }

    private static string Clean(string text)
    {
        text = Regex.Replace(text, "<xref href=\"([^\"]*)\"[^>]*>(</xref>)?", m => "`" + Display(m.Groups[1].Value) + "`");
        text = Regex.Replace(text, "<code class=\"(?:paramref|typeparamref)\">([^<]*)</code>", "`$1`");
        return text.Replace("\r\n", "\n").Trim();
    }

    private static void CrossCheck(Context context, List<Entry> items)
    {
        if (items.Count == 0)
        {
            throw new Failure("the library has no public API; docfx found no items");
        }
        List<MetadataReference> references = new List<MetadataReference>();
        foreach (string path in context.FrameworkRefs.Concat(context.References))
        {
            references.Add(MetadataReference.CreateFromFile(path));
        }
        MetadataReference own = MetadataReference.CreateFromFile(context.OwnRef);
        references.Add(own);
        CSharpCompilation compilation = CSharpCompilation.Create(
            "dx-ref-check",
            references: references,
            options: new CSharpCompilationOptions(OutputKind.DynamicallyLinkedLibrary, metadataImportOptions: MetadataImportOptions.Public));
        IAssemblySymbol assembly = (IAssemblySymbol)compilation.GetAssemblyOrModuleSymbol(own);
        HashSet<string> expected = new HashSet<string>(StringComparer.Ordinal);
        CollectNamespace(assembly.GlobalNamespace, expected);
        HashSet<string> actual = new HashSet<string>(items.Select(i => i.DocId), StringComparer.Ordinal);
        List<string> missing = expected.Where(id => !actual.Contains(id)).OrderBy(id => id, StringComparer.Ordinal).ToList();
        List<string> extra = actual.Where(id => !expected.Contains(id)).OrderBy(id => id, StringComparer.Ordinal).ToList();
        if (missing.Count > 0 || extra.Count > 0)
        {
            throw new Failure("docfx and the compiled reference assembly disagree about the public API; missing from docfx: [" + string.Join(", ", missing) + "]; not in the assembly: [" + string.Join(", ", extra) + "]");
        }
    }

    private static void Locate(Context context, List<Entry> items)
    {
        CSharpParseOptions parse = new CSharpParseOptions(LanguageVersion.Latest, DocumentationMode.Parse, SourceCodeKind.Regular, context.Defines);
        List<SyntaxTree> trees = new List<SyntaxTree>();
        foreach (string src in context.Srcs)
        {
            trees.Add(CSharpSyntaxTree.ParseText(File.ReadAllText(src), parse, src));
        }
        List<MetadataReference> references = new List<MetadataReference>();
        foreach (string path in context.FrameworkRefs.Concat(context.References))
        {
            references.Add(MetadataReference.CreateFromFile(path));
        }
        CSharpCompilation compilation = CSharpCompilation.Create(
            "dx-source-locate",
            trees,
            references,
            new CSharpCompilationOptions(OutputKind.DynamicallyLinkedLibrary, allowUnsafe: context.AllowUnsafe));
        foreach (Entry entry in items)
        {
            if (entry.DocId.StartsWith("N:", StringComparison.Ordinal) || entry.File != "")
            {
                continue;
            }
            ISymbol symbol = DocumentationCommentId.GetFirstSymbolForDeclarationId(entry.DocId, compilation);
            Location location = symbol == null ? null : symbol.Locations.FirstOrDefault(l => l.IsInSource);
            if (location == null)
            {
                throw new Failure("'" + entry.DocId + "' has no source location");
            }
            FileLinePositionSpan span = location.GetLineSpan();
            entry.File = location.SourceTree.FilePath.Replace('\\', '/');
            entry.Line = span.StartLinePosition.Line + 1;
        }
    }

    private static bool Visible(ISymbol symbol)
    {
        return symbol.DeclaredAccessibility == Accessibility.Public
            || symbol.DeclaredAccessibility == Accessibility.Protected
            || symbol.DeclaredAccessibility == Accessibility.ProtectedOrInternal;
    }

    private static void CollectNamespace(INamespaceSymbol ns, HashSet<string> ids)
    {
        foreach (ISymbol member in ns.GetMembers())
        {
            INamespaceSymbol child = member as INamespaceSymbol;
            INamedTypeSymbol type = member as INamedTypeSymbol;
            if (child != null)
            {
                CollectNamespace(child, ids);
            }
            else if (type != null && type.DeclaredAccessibility == Accessibility.Public)
            {
                ids.Add(type.ContainingNamespace.GetDocumentationCommentId());
                CollectType(type, ids);
            }
        }
    }

    private static void CollectType(INamedTypeSymbol type, HashSet<string> ids)
    {
        ids.Add(type.GetDocumentationCommentId());
        if (type.TypeKind == TypeKind.Delegate)
        {
            return;
        }
        foreach (ISymbol member in type.GetMembers())
        {
            INamedTypeSymbol nested = member as INamedTypeSymbol;
            IMethodSymbol method = member as IMethodSymbol;
            if (!Visible(member))
            {
                continue;
            }
            if (nested != null)
            {
                CollectType(nested, ids);
                continue;
            }
            if (method != null && (method.MethodKind == MethodKind.PropertyGet
                || method.MethodKind == MethodKind.PropertySet
                || method.MethodKind == MethodKind.EventAdd
                || method.MethodKind == MethodKind.EventRemove
                || method.MethodKind == MethodKind.EventRaise
                || method.MethodKind == MethodKind.StaticConstructor
                || method.MethodKind == MethodKind.ExplicitInterfaceImplementation))
            {
                continue;
            }
            if (method != null && method.MethodKind == MethodKind.Constructor && method.Parameters.Length == 0
                && (type.TypeKind == TypeKind.Struct || type.TypeKind == TypeKind.Enum))
            {
                continue;
            }
            if (member.Name == "value__")
            {
                continue;
            }
            string id = member.GetDocumentationCommentId();
            if (id != null)
            {
                ids.Add(id);
            }
        }
    }

    private static string Render(Context context, List<Entry> items)
    {
        using MemoryStream stream = new MemoryStream();
        JsonWriterOptions options = new JsonWriterOptions
        {
            Indented = true,
            NewLine = "\n",
            Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
        };
        using (Utf8JsonWriter writer = new Utf8JsonWriter(stream, options))
        {
            writer.WriteStartObject();
            writer.WriteString("schema", Schema);
            writer.WriteString("language", "csharp");
            writer.WriteString("assembly", context.Assembly);
            writer.WriteStartArray("items");
            foreach (Entry entry in items)
            {
                WriteEntry(writer, entry);
            }
            writer.WriteEndArray();
            writer.WriteEndObject();
        }
        return Encoding.UTF8.GetString(stream.ToArray()) + "\n";
    }

    private static void WriteEntry(Utf8JsonWriter writer, Entry entry)
    {
        writer.WriteStartObject();
        writer.WriteString("doc_id", entry.DocId);
        writer.WriteString("kind", entry.Kind);
        writer.WriteString("name", entry.Name);
        writer.WriteString("display_name", entry.DisplayName);
        if (entry.Parent == "")
        {
            writer.WriteNull("parent_doc_id");
        }
        else
        {
            writer.WriteString("parent_doc_id", entry.Parent);
        }
        writer.WriteString("signature", entry.Signature);
        writer.WriteString("summary", entry.Summary);
        writer.WriteString("remarks", entry.Remarks);
        writer.WriteStartArray("params");
        foreach (string[] parameter in entry.Params)
        {
            writer.WriteStartObject();
            writer.WriteString("name", parameter[0]);
            writer.WriteString("type", parameter[1]);
            writer.WriteString("doc", parameter[2]);
            writer.WriteEndObject();
        }
        writer.WriteEndArray();
        if (entry.Returns == null)
        {
            writer.WriteNull("returns");
        }
        else
        {
            writer.WriteStartObject("returns");
            writer.WriteString("type", entry.Returns[0]);
            writer.WriteString("doc", entry.Returns[1]);
            writer.WriteEndObject();
        }
        writer.WriteStartArray("examples");
        foreach (string example in entry.Examples)
        {
            writer.WriteStringValue(example);
        }
        writer.WriteEndArray();
        writer.WriteString("file", entry.File);
        writer.WriteNumber("line", entry.Line);
        writer.WriteStartArray("implements");
        foreach (string target in entry.Implements.OrderBy(t => t, StringComparer.Ordinal))
        {
            writer.WriteStringValue(target);
        }
        writer.WriteEndArray();
        writer.WriteEndObject();
    }
}
