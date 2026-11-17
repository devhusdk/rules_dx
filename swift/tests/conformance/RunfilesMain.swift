import BazelRunfiles
import Foundation

@main
struct RunfilesMain {
  static func main() throws {
    let runfiles = try Runfiles.create()
    let url = try runfiles.rlocation(CommandLine.arguments[1])
    print(try String(contentsOf: url, encoding: .utf8), terminator: "")
  }
}
