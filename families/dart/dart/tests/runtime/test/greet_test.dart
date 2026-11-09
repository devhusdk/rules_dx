import 'dart:io';

import 'package:greet/greet.dart';
import 'package:runfiles/runfiles.dart';
import 'package:test/test.dart';

void main() {
  test('greets by name', () {
    expect(greet('hello', 'dart'), 'hello dart');
  });

  test('reads data through runfiles', () {
    final path = Runfiles.create().rlocation('_main/dart/tests/runtime/data/name.txt');
    expect(File(path).readAsStringSync().trim(), 'dart');
  });
}
