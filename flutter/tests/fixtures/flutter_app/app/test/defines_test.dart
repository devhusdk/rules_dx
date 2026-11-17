import 'package:flutter_test/flutter_test.dart';

void main() {
  test('dart_defines reach String.fromEnvironment', () {
    expect(const String.fromEnvironment('PARTS_MODE'), 'nondefault');
    expect(const String.fromEnvironment('PARTS_UNSET', defaultValue: 'fallback'), 'fallback');
  });
}
