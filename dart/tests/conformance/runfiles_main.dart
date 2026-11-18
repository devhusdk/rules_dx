import 'dart:io';

import 'package:runfiles/runfiles.dart';

void main(List<String> args) {
  stdout.write(File(Runfiles.create().rlocation(args[0])).readAsStringSync());
}
