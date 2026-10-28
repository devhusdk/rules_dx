import 'schema.g.dart';

export 'schema.g.dart';

/// One line of the inventory.
class Item {
  const Item(this.name, this.qty, this.priceCents);

  final String name;
  final int qty;
  final int priceCents;

  int get totalCents => qty * priceCents;

  /// The CSV header this model reads, taken from the generated schema.
  static String get header => schemaFields.join(',');
}
