// Runs a tinybench `Bench` exported by the module named in argv[2] and writes dx-bench.json.
import { mkdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

const target = process.argv[2];
if (!target) {
  console.error("usage: bench_driver.mjs <bench module>");
  process.exit(2);
}

const mod = await import(pathToFileURL(path.resolve(target)).href);
const bench = mod.default ?? mod.bench;
if (!bench || typeof bench.run !== "function") {
  console.error(`${target} must export a tinybench Bench as its default export`);
  process.exit(2);
}

await bench.run();

const results = [];
for (const task of bench.tasks) {
  const result = task.result;
  if (result?.state !== "completed") {
    console.error(`benchmark ${task.name} did not complete: ${result?.state}`);
    process.exit(1);
  }
  results.push({
    name: task.name,
    unit: "ms",
    mean: result.latency.mean,
    stddev: result.latency.sd,
    iterations: result.latency.samplesCount,
  });
}

const dir = process.env.TEST_UNDECLARED_OUTPUTS_DIR;
if (dir) {
  mkdirSync(dir, { recursive: true });
  writeFileSync(path.join(dir, "dx-bench.json"), JSON.stringify({ schema: "dx-bench/v1", results }));
}
console.table(results);
