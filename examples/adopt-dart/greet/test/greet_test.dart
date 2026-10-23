import 'package:greet/greet.dart';
import 'package:test/test.dart';

void main() {
  test('greets each name once in order', () {
    expect(greetAll(['bo', 'al', 'bo']), ['hello al', 'hello bo']);
  });
}
