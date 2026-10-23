import 'package:tally/tally.dart';
import 'package:test/test.dart';

void main() {
  test('sums', () => expect(tally([1, 2]), 3));
}
