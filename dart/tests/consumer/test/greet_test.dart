import 'dart:io';

import 'package:consumer/greet.dart';

void main() {
  if (greet('world') != 'hello world') {
    exit(1);
  }
  stdout.writeln('PASS');
}
