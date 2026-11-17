import ArgumentParser

@main
struct LockMain: ParsableCommand {
  @Option var name = "lock"

  func run() {
    print("hello \(name)")
  }
}
