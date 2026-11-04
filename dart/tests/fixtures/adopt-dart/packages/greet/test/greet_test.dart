import 'package:greet/greet.dart';
import 'package:test/test.dart';

void main() {
  test('greets by name', () {
    expect(greet('dart'), 'hello dart');
  });
}
