import 'package:collection/collection.dart';

import 'categories.g.dart';
import 'entry.dart';

/// Sums [entries] per category, largest total first, ties by name.
List<(Category, int)> totals(Iterable<Entry> entries) {
  final groups = groupBy(entries, (Entry entry) => entry.category);
  return [
    for (final MapEntry(:key, :value) in groups.entries)
      (key, value.map((entry) => entry.cents).sum),
  ].sorted((a, b) {
    final byTotal = b.$2.compareTo(a.$2);
    return byTotal != 0 ? byTotal : a.$1.name.compareTo(b.$1.name);
  });
}

/// Formats [cents] as a decimal amount, like `12.05`.
String formatCents(int cents) {
  final whole = cents ~/ 100;
  final rest = (cents % 100).toString().padLeft(2, '0');
  return '$whole.$rest';
}
