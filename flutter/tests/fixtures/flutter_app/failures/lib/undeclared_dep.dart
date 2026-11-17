import 'package:collection/collection.dart';
import 'package:flutter/widgets.dart';

void main() {
  runApp(Text([3, 1, 2].sorted((a, b) => a - b).join(), textDirection: TextDirection.ltr));
}
