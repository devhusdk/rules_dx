import 'dart:io';

import 'package:ledger/ledger.dart';
import 'package:model/model.dart';
import 'package:runfiles/runfiles.dart';
import 'package:test/test.dart';

void main() {
  test('reads the data file through runfiles', () {
    final path = Runfiles.create().rlocation('_main/examples/dart-app/ledger/data/items.csv');
    final items = loadItems(path);
    expect(items.map((item) => item.name), ['adapter', 'battery', 'cable']);
    expect(totalCents(items), 12 * 450 + 3 * 1299 + 40 * 199);
    expect(File(path).existsSync(), isTrue);
  });

  test('the generated schema is current', () {
    expect(schemaVersion, 1);
    expect(Item.header, 'name,qty,price_cents');
  });

  test('rejects a malformed row', () {
    expect(
      () => parseItems('name,qty,price_cents\ncable,twelve,450\n'),
      throwsA(isA<LedgerException>()),
    );
  });

  test('rejects a wrong header', () {
    expect(() => parseItems('a,b\n1,2\n'), throwsA(isA<LedgerException>()));
  });
}
