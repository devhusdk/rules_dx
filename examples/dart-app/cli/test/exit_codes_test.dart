import 'dart:io';

import 'package:runfiles/runfiles.dart';
import 'package:test/test.dart';

ProcessResult runInventory(List<String> arguments) {
  final suffix = Platform.isWindows ? '.exe' : '';
  final bin = Runfiles.create().rlocation('_main/examples/dart-app/cli/inventory$suffix');
  return Process.runSync(bin, arguments);
}

void main() {
  test('exit 0 lists the items', () {
    final result = runInventory(const []);
    expect(result.exitCode, 0, reason: '${result.stderr}');
    expect(result.stdout, contains('cable 12 450'));
    expect(result.stdout, contains('schema 1'));
  });

  test('exit 0 for a known item', () {
    final result = runInventory(const ['get', 'cable']);
    expect(result.exitCode, 0, reason: '${result.stderr}');
    expect((result.stdout as String).trim(), 'cable 12 450');
  });

  test('exit 2 for bad usage', () {
    expect(runInventory(const ['get']).exitCode, 2);
    expect(runInventory(const ['--nope']).exitCode, 2);
  });

  test('exit 4 for an unknown item', () {
    final result = runInventory(const ['get', 'missing']);
    expect(result.exitCode, 4);
    expect(result.stderr, contains('unknown item: missing'));
  });
}
