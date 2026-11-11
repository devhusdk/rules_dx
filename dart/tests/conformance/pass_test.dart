import 'package:conformance/greet.dart';
import 'package:test/test.dart';

void main() {
  test('passing_case', () {
    expect(greet('world'), 'hello world');
  });
}
