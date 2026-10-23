import 'package:ledger/ledger.dart';
import 'package:test/test.dart';

void main() {
  test('sums per category, largest first, ties by name', () {
    final sums = totals(const [
      Entry('2026-01-01', Category.food, 300),
      Entry('2026-01-02', Category.travel, 500),
      Entry('2026-01-03', Category.food, 200),
      Entry('2026-01-04', Category.rent, 100),
      Entry('2026-01-05', Category.utilities, 100),
    ]);
    expect(sums, [
      (Category.food, 500),
      (Category.travel, 500),
      (Category.rent, 100),
      (Category.utilities, 100),
    ]);
  });

  test('formats cents with two decimals', () {
    expect(formatCents(5), '0.05');
    expect(formatCents(120000), '1200.00');
    expect(formatCents(1875), '18.75');
  });

  test('generated categories follow the source list', () {
    expect(Category.values.map((c) => c.name), [
      'food',
      'rent',
      'travel',
      'utilities',
    ]);
    expect(Category.named('travel'), Category.travel);
    expect(Category.named('toys'), isNull);
  });
}
