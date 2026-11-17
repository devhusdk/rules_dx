import 'dart:io';

import 'package:collection/collection.dart';
import 'package:model/model.dart';

/// Raised when the CSV does not match the generated schema.
class LedgerException implements Exception {
  const LedgerException(this.message);

  final String message;

  @override
  String toString() => 'LedgerException: $message';
}

/// Parses [text] and returns the items sorted by name.
List<Item> parseItems(String text) {
  final lines = text.split('\n').map((line) => line.trim()).where((line) => line.isNotEmpty).toList();
  if (lines.isEmpty || lines.first != Item.header) {
    throw const LedgerException('missing or unexpected header');
  }
  final items = <Item>[];
  for (final (index, line) in lines.skip(1).indexed) {
    final cells = line.split(',');
    final qty = cells.length == 3 ? int.tryParse(cells[1]) : null;
    final price = cells.length == 3 ? int.tryParse(cells[2]) : null;
    if (qty == null || price == null) {
      throw LedgerException('malformed row ${index + 2}: $line');
    }
    items.add(Item(cells[0], qty, price));
  }
  return items.sortedBy((item) => item.name);
}

/// Reads and parses the CSV at [path].
List<Item> loadItems(String path) => parseItems(File(path).readAsStringSync());

/// The sum of every item's total, in cents.
int totalCents(Iterable<Item> items) => items.map((item) => item.totalCents).sum;
