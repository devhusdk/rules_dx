import 'package:conformance/greet.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('failing_case', () {
    expect(greet('world'), 'goodbye world');
  });
}
