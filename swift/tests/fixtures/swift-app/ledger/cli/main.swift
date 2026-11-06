import ArgumentParser
import LedgerCore

#if LEDGER_STRICT
  let mode = "strict"
#else
  let mode = "lenient"
#endif

struct Ledger: ParsableCommand {
  @Argument(help: "Entries as name=cents.")
  var entries: [String] = []

  mutating func run() throws {
    var parsed: [Entry] = []
    for text in entries {
      let parts = text.split(separator: "=", maxSplits: 1)
      guard parts.count == 2, let cents = Int(parts[1]) else {
        throw ValidationError("bad entry '\(text)', want name=cents")
      }
      parsed.append(Entry(name: String(parts[0]), cents: cents))
    }
    print("mode: \(mode)")
    print("total: \(total(parsed))")
    print("digest: \(digest(entries.joined(separator: ",")))")
  }
}

Ledger.main()
