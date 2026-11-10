using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;
using BenchmarkDotNet.Configs;
using BenchmarkDotNet.Exporters.Json;
using BenchmarkDotNet.Jobs;
using BenchmarkDotNet.Running;
using BenchmarkDotNet.Toolchains.InProcess.NoEmit;

namespace DxBench
{
    internal static class BenchMain
    {
        public static int Main(string[] args)
        {
            var outDir = Environment.GetEnvironmentVariable("TEST_UNDECLARED_OUTPUTS_DIR") ?? Directory.GetCurrentDirectory();
            var artifacts = Path.Combine(outDir, "BenchmarkDotNet.Artifacts");
            var config = DefaultConfig.Instance
                .AddJob(Job.ShortRun.WithToolchain(InProcessNoEmitToolchain.Instance))
                .AddExporter(JsonExporter.Full)
                .WithOption(ConfigOptions.DisableOptimizationsValidator, true)
                .WithArtifactsPath(artifacts);
            BenchmarkSwitcher.FromAssembly(typeof(BenchMain).Assembly).Run(args.Length == 0 ? new[] { "--filter", "*" } : args, config);
            return Convert(Path.Combine(artifacts, "results"), Path.Combine(outDir, "dx-bench.json"));
        }

        private static int Convert(string resultsDir, string output)
        {
            var rows = new List<Dictionary<string, object>>();
            var reports = Directory.Exists(resultsDir) ? Directory.GetFiles(resultsDir, "*-report-full.json") : Array.Empty<string>();
            foreach (var report in reports.OrderBy(p => p, StringComparer.Ordinal))
            {
                using var doc = JsonDocument.Parse(File.ReadAllText(report));
                foreach (var bench in doc.RootElement.GetProperty("Benchmarks").EnumerateArray())
                {
                    if (!bench.TryGetProperty("Statistics", out var stats) || stats.ValueKind != JsonValueKind.Object)
                        continue;
                    rows.Add(new Dictionary<string, object>
                    {
                        ["name"] = bench.GetProperty("FullName").GetString() ?? "",
                        ["unit"] = "ns",
                        ["mean"] = stats.GetProperty("Mean").GetDouble(),
                        ["stddev"] = stats.GetProperty("StandardDeviation").GetDouble(),
                        ["iterations"] = stats.GetProperty("N").GetInt32(),
                    });
                }
            }
            if (rows.Count == 0)
            {
                Console.Error.WriteLine("BenchmarkDotNet wrote no results under " + resultsDir);
                return 1;
            }
            File.WriteAllText(output, JsonSerializer.Serialize(new Dictionary<string, object>
            {
                ["schema"] = "dx-bench/v1",
                ["results"] = rows,
            }));
            return 0;
        }
    }
}
