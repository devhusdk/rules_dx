import 'package:ledger/ledger.dart';
import 'package:test/test.dart';

void main() {
  test('parses entries in file order', () {
    final entries = parseLedger(
      'date,category,cents\n2026-01-02,rent,120000\n\n2026-01-03,food,2350\n',
    );
    expect(entries.map((e) => (e.date, e.category, e.cents)), [
      ('2026-01-02', Category.rent, 120000),
      ('2026-01-03', Category.food, 2350),
    ]);
  });

  test('accepts CRLF line endings', () {
    final entries = parseLedger('date,category,cents\r\n2026-01-03,food,1\r\n');
    expect(entries.single.cents, 1);
  });

  final rejected = {
    'when,what\n2026-01-02,rent,1\n': 'line 1: header must be "$ledgerHeader"',
    'date,category,cents\n': 'ledger has no entries',
    'date,category,cents\n2026-01-02,toys,5\n': 'line 2: unknown category toys',
    'date,category,cents\n2026-01-02,food,0\n':
        'line 2: cents must be a positive whole number',
    'date,category,cents\n2026-01-02,food\n': 'line 2: want 3 fields, got 2',
    'date,category,cents\nJan 2,food,5\n': 'line 2: date must be YYYY-MM-DD',
  };
  for (final MapEntry(key: text, value: reason) in rejected.entries) {
    test('rejects with "$reason"', () {
      expect(
        () => parseLedger(text),
        throwsA(
          isA<LedgerException>().having((e) => '$e', 'message', reason),
        ),
      );
    });
  }
}
