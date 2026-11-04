import 'dart:io';

import 'package:inventory_cli/inventory_cli.dart';
import 'package:runfiles/runfiles.dart';

void main(List<String> arguments) {
  final csv = Runfiles.create().rlocation('_main/dart/tests/fixtures/dart-app/ledger/data/items.csv');
  exit(run(arguments, csv, stdout.writeln, stderr.writeln));
}
