module DxBench.BenchMain

open System
open System.Collections.Generic
open System.IO
open System.Linq
open System.Reflection
open System.Text.Json
open BenchmarkDotNet.Configs
open BenchmarkDotNet.Exporters.Json
open BenchmarkDotNet.Jobs
open BenchmarkDotNet.Running
open BenchmarkDotNet.Toolchains.InProcess.NoEmit

let private convert (resultsDir: string) (output: string) =
    let rows = List<Dictionary<string, obj>>()

    let reports =
        if Directory.Exists resultsDir then
            Directory.GetFiles(resultsDir, "*-report-full.json")
        else
            Array.empty

    for report in reports.OrderBy((fun p -> p), StringComparer.Ordinal) do
        use doc = JsonDocument.Parse(File.ReadAllText report)

        for bench in doc.RootElement.GetProperty("Benchmarks").EnumerateArray() do
            let mutable stats = Unchecked.defaultof<JsonElement>

            if bench.TryGetProperty("Statistics", &stats) && stats.ValueKind = JsonValueKind.Object then
                let row = Dictionary<string, obj>()
                row["name"] <- box (bench.GetProperty("FullName").GetString() |> Option.ofObj |> Option.defaultValue "")
                row["unit"] <- box "ns"
                row["mean"] <- box (stats.GetProperty("Mean").GetDouble())
                row["stddev"] <- box (stats.GetProperty("StandardDeviation").GetDouble())
                row["iterations"] <- box (stats.GetProperty("N").GetInt32())
                rows.Add row

    if rows.Count = 0 then
        eprintfn "BenchmarkDotNet wrote no results under %s" resultsDir
        1
    else
        let doc = Dictionary<string, obj>()
        doc["schema"] <- box "dx-bench/v1"
        doc["results"] <- box rows
        File.WriteAllText(output, JsonSerializer.Serialize doc)
        0

[<EntryPoint>]
let main args =
    let outDir =
        match Environment.GetEnvironmentVariable "TEST_UNDECLARED_OUTPUTS_DIR" with
        | null -> Directory.GetCurrentDirectory()
        | dir -> dir

    let artifacts = Path.Combine(outDir, "BenchmarkDotNet.Artifacts")

    let config =
        DefaultConfig.Instance
            .AddJob(Job.ShortRun.WithToolchain(InProcessNoEmitToolchain.Instance))
            .AddExporter(JsonExporter.Full)
            .WithOption(ConfigOptions.DisableOptimizationsValidator, true)
            .WithArtifactsPath(artifacts)

    let args = if args.Length = 0 then [| "--filter"; "*" |] else args
    BenchmarkSwitcher.FromAssembly(Assembly.GetExecutingAssembly()).Run(args, config) |> ignore
    convert (Path.Combine(artifacts, "results")) (Path.Combine(outDir, "dx-bench.json"))
