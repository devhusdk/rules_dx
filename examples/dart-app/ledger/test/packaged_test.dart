import 'dart:io';

import 'package:ledger/ledger.dart';
import 'package:runfiles/runfiles.dart';
import 'package:test/test.dart';

void main() {
  test('packaged ledger is a runfile and parses', () {
    final path = Runfiles.create().rlocation(
      '_main/examples/dart-app/ledger/entries.csv',
    );
    final entries = parseLedger(File(path).readAsStringSync());
    expect(entries, hasLength(6));
    expect(formatCents(totals(entries).first.$2), '1200.00');
  });
}
