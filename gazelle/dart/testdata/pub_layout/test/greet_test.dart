import 'package:greet/greet.dart';
import 'package:test/test.dart';

import 'helpers.dart';

void main() => test('greets', () => expect(greet(name), isNotEmpty));
