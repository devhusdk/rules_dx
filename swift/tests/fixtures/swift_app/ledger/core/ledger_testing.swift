import LedgerCore
import Testing

@Test func totalAddsCents() {
  #expect(total([Entry(name: "tea", cents: 250), Entry(name: "milk", cents: 120)]) == 370)
}

@Test func digestMatchesFnv1a() {
  #expect(digest("") == 0x811C_9DC5)
  #expect(digest("a") == 0xE40C_292C)
}
