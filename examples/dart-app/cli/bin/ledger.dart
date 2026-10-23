import 'dart:io';

import 'package:ledger_cli/cli.dart';
import 'package:runfiles/runfiles.dart';

const _packaged = String.fromEnvironment('LEDGER_DATA');

String _read(String? path) {
  final chosen = path ?? Platform.environment['LEDGER_FILE'];
  if (chosen != null) {
    final base = Platform.environment['BUILD_WORKING_DIRECTORY'];
    final file = File(chosen);
    return (base == null || file.isAbsolute ? file : File('$base/$chosen'))
        .readAsStringSync();
  }
  return File(Runfiles.create().rlocation(_packaged)).readAsStringSync();
}

void main(List<String> arguments) {
  exitCode = run(arguments, out: stdout, err: stderr, readLedger: _read);
}
