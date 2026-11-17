import 'package:collection/collection.dart';
import 'package:flutter/foundation.dart';

/// The parts the user has added, newest first.
class PartsModel extends ChangeNotifier {
  PartsModel(this._catalog);

  final List<String> _catalog;
  final List<String> _parts = <String>[];

  UnmodifiableListView<String> get parts => UnmodifiableListView(_parts);

  int get count => _parts.length;

  Map<String, int> get totals => _parts.groupFoldBy<String, int>((part) => part, (total, _) => (total ?? 0) + 1);

  void add() {
    if (_catalog.isEmpty) {
      return;
    }
    _parts.insert(0, _catalog[_parts.length % _catalog.length]);
    notifyListeners();
  }
}
