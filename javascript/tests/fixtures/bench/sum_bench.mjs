import { Bench } from "tinybench";

const bench = new Bench({ time: 20, warmupTime: 0, warmupIterations: 1, iterations: 3 });
const numbers = Array.from({ length: 1000 }, (_, i) => i);

bench.add("sum", () => numbers.reduce((a, b) => a + b, 0));

export default bench;
