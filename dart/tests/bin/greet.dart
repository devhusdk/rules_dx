import 'dart:io';

import 'package:greet/greet.dart';

void main(List<String> args) {
  stdout.writeln(greet(args.isEmpty ? 'world' : args.first));
}
