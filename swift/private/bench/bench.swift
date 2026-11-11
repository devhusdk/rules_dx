import Foundation

/// Times `body` and writes the result to `$TEST_UNDECLARED_OUTPUTS_DIR/dx-bench.json`.
public func dxBench(_ name: String, iterations: Int = 1000, _ body: () -> Void) {
    var samples: [Double] = []
    for _ in 0..<10 {
        let start = DispatchTime.now().uptimeNanoseconds
        for _ in 0..<iterations { body() }
        samples.append(Double(DispatchTime.now().uptimeNanoseconds - start) / Double(iterations))
    }
    let mean = samples.reduce(0, +) / Double(samples.count)
    let variance = samples.map { ($0 - mean) * ($0 - mean) }.reduce(0, +) / Double(samples.count)
    let json =
        "{\"schema\":\"dx-bench/v1\",\"results\":[{\"name\":\"\(name)\",\"unit\":\"ns/iter\","
        + "\"mean\":\(mean),\"stddev\":\(variance.squareRoot()),\"iterations\":\(iterations * samples.count)}]}\n"
    let dir = ProcessInfo.processInfo.environment["TEST_UNDECLARED_OUTPUTS_DIR"] ?? "."
    try? FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)
    try! json.write(toFile: dir + "/dx-bench.json", atomically: true, encoding: .utf8)
    print("\(name): \(mean) ns/iter")
}
