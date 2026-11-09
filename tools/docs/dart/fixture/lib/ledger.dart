/// Ledger entries and balances.
library;

/// A running balance of added entries.
class Ledger {
  /// Creates an empty ledger.
  Ledger();

  /// The current balance.
  int balance = 0;

  /// Adds [amount] to the balance and returns the new balance.
  int add(int amount) => balance += amount;
}

/// Formats [amount] for display.
String format(int amount) => '$amount';

/// The largest amount one entry may hold.
const int maxAmount = 1000000;

/// The entry kinds.
enum Kind {
  /// Money in.
  credit,

  /// Money out.
  debit,
}
