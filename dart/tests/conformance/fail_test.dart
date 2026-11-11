import 'package:conformance/greet.dart';
import 'package:test/test.dart';

void main() {
  test('failing_case', () {
    expect(greet('world'), 'goodbye world');
  });
}
