using System;
using System.Collections.Generic;
using System.IO;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Runtime.Loader;
using System.Text;
using System.Text.Json;

namespace Dx.Quality.Roslyn;

/// <summary>
/// Runs the SDK's csc.dll in-process, as a one-shot command or a Bazel persistent worker.
/// </summary>
/// <remarks>
/// One-shot: <c>roslyn_worker CSC_DLL @params</c>. Worker: Bazel starts
/// <c>roslyn_worker CSC_DLL --persistent_worker</c> and sends one compact JSON <c>WorkRequest</c>
/// per line on stdin, whose <c>arguments</c> are the params file lines. Requests arrive one at a
/// time (singleplex) and, under <c>--worker_sandboxing</c>, the working directory is the worker's
/// sandbox.
///
/// Each request is <c>--status PATH</c> followed by the csc arguments, one per line. The csc exit
/// code is written to PATH and the request itself succeeds, so a failing compilation is a finding
/// rather than a failed action.
/// </remarks>
public static class Program
{
    private const string Usage = "usage: roslyn_worker CSC_DLL @params | CSC_DLL --persistent_worker";

    public static int Main(string[] args)
    {
        if (args.Length != 2)
        {
            Console.Error.WriteLine(Usage);
            return 2;
        }
        Compiler csc;
        try
        {
            csc = new Compiler(Path.GetFullPath(args[0]));
        }
        catch (Exception err) when (err is IOException or BadImageFormatException or InvalidOperationException or MissingMemberException)
        {
            Console.Error.WriteLine($"roslyn_worker: {args[0]}: {err.Message}");
            return 1;
        }
        if (args[1] == "--persistent_worker")
        {
            return Serve(Console.In, Console.OpenStandardOutput(), csc);
        }
        if (!args[1].StartsWith('@'))
        {
            Console.Error.WriteLine(Usage);
            return 2;
        }
        string[] lines;
        try
        {
            lines = File.ReadAllLines(args[1][1..]);
        }
        catch (Exception err) when (err is IOException or UnauthorizedAccessException)
        {
            Console.Error.WriteLine($"roslyn_worker: {args[1][1..]}: {err.Message}");
            return 1;
        }
        var (exitCode, output) = Handle(lines, new Dictionary<string, string>(), csc);
        Console.Out.Write(output);
        return exitCode;
    }

    /// <summary>Answers work requests from <paramref name="input"/> on <paramref name="protocol"/> until input ends.</summary>
    public static int Serve(TextReader input, Stream protocol, Compiler csc)
    {
        string? line;
        while ((line = input.ReadLine()) != null)
        {
            if (line.Length == 0)
            {
                continue;
            }
            var request = JsonDocument.Parse(line).RootElement;
            var lines = new List<string>();
            if (request.TryGetProperty("arguments", out var arguments))
            {
                foreach (var argument in arguments.EnumerateArray())
                {
                    lines.Add(argument.GetString() ?? "");
                }
            }
            var digests = new Dictionary<string, string>();
            if (request.TryGetProperty("inputs", out var inputs))
            {
                foreach (var file in inputs.EnumerateArray())
                {
                    if (file.TryGetProperty("path", out var path) && file.TryGetProperty("digest", out var digest))
                    {
                        digests[path.GetString() ?? ""] = digest.GetString() ?? "";
                    }
                }
            }
            // JSON omits a zero requestId, as singleplex requests always have.
            var id = request.TryGetProperty("requestId", out var requestId) ? requestId.GetInt32() : 0;
            var (exitCode, output) = Handle(lines, digests, csc);
            using (var writer = new Utf8JsonWriter(protocol))
            {
                writer.WriteStartObject();
                writer.WriteNumber("exitCode", exitCode);
                writer.WriteString("output", output);
                writer.WriteNumber("requestId", id);
                writer.WriteEndObject();
            }
            protocol.WriteByte((byte)'\n');
            protocol.Flush();
        }
        return 0;
    }

    /// <summary>Runs one request and returns the request's exit code and its console output.</summary>
    /// <param name="digests">Input path to content digest, empty when Bazel sent none.</param>
    public static (int ExitCode, string Output) Handle(IReadOnlyList<string> lines, IReadOnlyDictionary<string, string> digests, Compiler csc)
    {
        if (lines.Count < 2 || lines[0] != "--status")
        {
            return (1, "roslyn_worker: request does not start with --status PATH\n");
        }
        var status = lines[1];
        var cscArgs = new List<string>(lines.Count - 1);
        for (var i = 2; i < lines.Count; i++)
        {
            cscArgs.Add(lines[i]);
        }
        var cwd = Directory.GetCurrentDirectory();
        // csc embeds absolute paths; map the execution root away as the rules_dotnet wrapper does.
        cscArgs.Add($"/pathmap:{cwd}=.");

        var captured = new StringWriter();
        var stdout = Console.Out;
        var stderr = Console.Error;
        int code;
        Console.SetOut(captured);
        Console.SetError(captured);
        try
        {
            code = csc.Run(cscArgs, cwd, captured, digests);
        }
        catch (TargetInvocationException err)
        {
            captured.WriteLine($"roslyn_worker: csc crashed: {err.InnerException}");
            code = 1;
        }
        finally
        {
            Console.SetOut(stdout);
            Console.SetError(stderr);
        }
        try
        {
            File.WriteAllText(status, $"{code}\n");
        }
        catch (Exception err) when (err is IOException or UnauthorizedAccessException)
        {
            captured.WriteLine($"roslyn_worker: {status}: {err.Message}");
            return (1, captured.ToString());
        }
        return (0, captured.ToString());
    }

    /// <summary>csc.dll loaded once, with an analyzer loader kept while the analyzers it loaded are unchanged.</summary>
    /// <remarks>
    /// csc's own entry point makes a fresh analyzer loader per run, whose load contexts are never
    /// collected, so a worker using it grows by every analyzer set on every request.
    /// </remarks>
    public sealed class Compiler
    {
        private readonly string clientDir;
        private readonly MethodInfo run;
        private readonly Type loaderType;
        private object? loader;
        private readonly Dictionary<string, string> loaded = new();

        public Compiler(string cscDll)
        {
            clientDir = Path.GetDirectoryName(cscDll)!;
            var context = new CompilerContext(clientDir);
            var program = context.LoadFromAssemblyPath(cscDll).GetType("Microsoft.CodeAnalysis.CSharp.CommandLine.Program", throwOnError: true)!;
            // Program.Run(args, clientDir, workingDir, sdkDir, tempDir, textWriter, analyzerLoader)
            run = program.GetMethod("Run", BindingFlags.Public | BindingFlags.Static, new[]
            {
                typeof(string[]), typeof(string), typeof(string), typeof(string), typeof(string), typeof(TextWriter),
                context.LoadFromAssemblyName(new AssemblyName("Microsoft.CodeAnalysis")).GetType("Microsoft.CodeAnalysis.IAnalyzerAssemblyLoader", throwOnError: true)!,
            }) ?? throw new MissingMethodException("Microsoft.CodeAnalysis.CSharp.CommandLine.Program", "Run");
            loaderType = context.LoadFromAssemblyName(new AssemblyName("Microsoft.CodeAnalysis")).GetType("Microsoft.CodeAnalysis.AnalyzerAssemblyLoader", throwOnError: true)!;
        }

        /// <summary>Compiles <paramref name="args"/> and returns csc's exit code.</summary>
        public int Run(List<string> args, string workingDir, TextWriter output, IReadOnlyDictionary<string, string> digests)
        {
            // The analyzers and every input beside them, which the loader resolves their dependencies from.
            var current = new Dictionary<string, string>();
            var dirs = new HashSet<string>();
            foreach (var arg in args)
            {
                if (arg.StartsWith("/analyzer:", StringComparison.Ordinal))
                {
                    var path = arg["/analyzer:".Length..];
                    current[path] = digests.GetValueOrDefault(path, "");
                    dirs.Add(Path.GetDirectoryName(path) ?? "");
                }
            }
            foreach (var (path, digest) in digests)
            {
                if (dirs.Contains(Path.GetDirectoryName(path) ?? ""))
                {
                    current[path] = digest;
                }
            }
            // A file rebuilt at a path the loader already read, or any file without a digest, needs a fresh loader.
            var stale = loader == null;
            foreach (var (path, digest) in current)
            {
                if (digest.Length == 0 || (loaded.TryGetValue(path, out var seen) && seen != digest))
                {
                    stale = true;
                }
            }
            if (stale)
            {
                loader = Activator.CreateInstance(loaderType, nonPublic: true);
                loaded.Clear();
            }
            foreach (var (path, digest) in current)
            {
                loaded[path] = digest;
            }
            return (int)run.Invoke(null, new object?[]
            {
                args.ToArray(), clientDir, workingDir, RuntimeEnvironment.GetRuntimeDirectory(), Path.GetTempPath(), output, loader,
            })!;
        }
    }

    /// <summary>Resolves the compiler's own assemblies from its directory, the rest from the shared framework.</summary>
    private sealed class CompilerContext(string directory) : AssemblyLoadContext("csc")
    {
        protected override Assembly? Load(AssemblyName name)
        {
            var path = Path.Combine(directory, name.Name + ".dll");
            return File.Exists(path) ? LoadFromAssemblyPath(path) : null;
        }
    }
}
