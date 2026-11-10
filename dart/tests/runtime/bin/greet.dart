import 'dart:io';

import 'package:greet/greet.dart';
import 'package:runfiles/runfiles.dart';

void main() {
  final path = Runfiles.create().rlocation('_main/dart/tests/runtime/data/name.txt');
  final name = File(path).readAsStringSync().trim();
  stdout.writeln(greet(const String.fromEnvironment('GREETING'), name));
}
