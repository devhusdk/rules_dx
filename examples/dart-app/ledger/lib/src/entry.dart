import 'categories.g.dart';

/// One ledger line: a date, a category and an amount in cents.
class Entry {
  /// Creates an entry.
  const Entry(this.date, this.category, this.cents);

  /// The day the money moved, as `YYYY-MM-DD`.
  final String date;

  /// What the money was spent on.
  final Category category;

  /// The amount in cents. Always positive.
  final int cents;
}
