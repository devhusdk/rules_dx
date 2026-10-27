import 'dart:io';

import 'package:runfiles/runfiles.dart';
import 'package:test/test.dart';

void main() {
  test('the binary prints the define and the runfiles data', () {
    final suffix = Platform.isWindows ? '.exe' : '';
    final bin = Runfiles.create().rlocation('_main/dart/tests/runtime/greet_bin$suffix');
    final result = Process.runSync(bin, const []);
    expect(result.exitCode, 0, reason: '${result.stderr}');
    expect((result.stdout as String).trim(), 'hi dart');
  });
}
