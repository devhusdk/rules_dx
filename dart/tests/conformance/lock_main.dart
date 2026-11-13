import 'package:collection/collection.dart';

void main() {
  print(['hello', 'lock'].whereNot((word) => word.isEmpty).join(' '));
}
