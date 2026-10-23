import 'package:collection/collection.dart';

/// Greets every name once, in sorted order.
List<String> greetAll(Iterable<String> names) =>
    names.toSet().sorted().map((name) => 'hello $name').toList();
