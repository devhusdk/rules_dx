import 'categories.g.dart';
import 'entry.dart';

/// The header every ledger file starts with.
const ledgerHeader = 'date,category,cents';

final _date = RegExp(r'^\d{4}-\d{2}-\d{2}$');

/// Thrown when a ledger file cannot be parsed.
class LedgerException implements Exception {
  /// Creates an exception for [line] with [message].
  const LedgerException(this.line, this.message);

  /// The 1-based line number, or 0 for the whole file.
  final int line;

  /// What is wrong.
  final String message;

  @override
  String toString() => line == 0 ? message : 'line $line: $message';
}

/// Parses ledger CSV [text] into entries, in file order.
List<Entry> parseLedger(String text) {
  final lines = text.replaceAll('\r\n', '\n').split('\n');
  if (lines.first.trim() != ledgerHeader) {
    throw const LedgerException(1, 'header must be "$ledgerHeader"');
  }
  final entries = <Entry>[];
  for (var i = 1; i < lines.length; i++) {
    final line = lines[i].trim();
    if (line.isEmpty) {
      continue;
    }
    entries.add(_parseLine(i + 1, line));
  }
  if (entries.isEmpty) {
    throw const LedgerException(0, 'ledger has no entries');
  }
  return entries;
}

Entry _parseLine(int number, String line) {
  final fields = line.split(',');
  if (fields.length != 3) {
    throw LedgerException(number, 'want 3 fields, got ${fields.length}');
  }
  final [date, name, amount] = fields;
  if (!_date.hasMatch(date)) {
    throw LedgerException(number, 'date must be YYYY-MM-DD');
  }
  final category = Category.named(name);
  if (category == null) {
    throw LedgerException(number, 'unknown category $name');
  }
  final cents = int.tryParse(amount);
  if (cents == null || cents <= 0) {
    throw LedgerException(number, 'cents must be a positive whole number');
  }
  return Entry(date, category, cents);
}
