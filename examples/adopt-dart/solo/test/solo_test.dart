import 'package:solo/solo.dart';

void main() {
  final got = reverseWords('a b c');
  if (got != 'c b a') {
    throw StateError('unexpected words: $got');
  }
}
