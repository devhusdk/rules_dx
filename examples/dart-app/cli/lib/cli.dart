/// The `ledger` command line: argument parsing, commands and exit codes.
library;

import 'dart:io';

import 'package:args/args.dart';
import 'package:ledger/ledger.dart';

/// The version stamped in at compile time with `-DLEDGER_VERSION`.
const ledgerVersion = String.fromEnvironment(
  'LEDGER_VERSION',
  defaultValue: 'dev',
);

/// Exit code for success.
const exitOk = 0;

/// Exit code for a usage error.
const exitUsage = 2;

/// Exit code for an unreadable or malformed ledger file.
const exitLedger = 3;

/// Exit code for an unknown category.
const exitCategory = 4;

const _usage = '''
usage: ledger [--file PATH] COMMAND
commands:
  totals          spending per category, largest first
  list            every entry in file order
  total CATEGORY  spending for one category
  categories      known categories''';

final _parser = ArgParser()
  ..addOption('file', help: 'ledger CSV to read')
  ..addFlag('version', negatable: false, help: 'print the version');

/// Runs the CLI with [arguments] and returns the exit code.
///
/// [readLedger] loads the text of the ledger at a path, or of the packaged
/// ledger when the path is null.
int run(
  List<String> arguments, {
  required StringSink out,
  required StringSink err,
  required String Function(String? path) readLedger,
}) {
  final ArgResults options;
  try {
    options = _parser.parse(arguments);
  } on FormatException catch (e) {
    err.writeln('ledger: ${e.message}');
    err.writeln(_usage);
    return exitUsage;
  }
  if (options.flag('version')) {
    out.writeln('ledger $ledgerVersion');
    return exitOk;
  }
  final rest = options.rest;
  if (rest.isEmpty) {
    err.writeln(_usage);
    return exitUsage;
  }
  final command = rest.first;
  final wantArgs = command == 'total' ? 1 : 0;
  if (!const {'totals', 'list', 'total', 'categories'}.contains(command) ||
      rest.length - 1 != wantArgs) {
    err.writeln(_usage);
    return exitUsage;
  }
  if (command == 'categories') {
    for (final category in Category.values) {
      out.writeln(category.name);
    }
    return exitOk;
  }
  Category? only;
  if (command == 'total') {
    only = Category.named(rest[1]);
    if (only == null) {
      err.writeln('ledger: no category ${rest[1]}');
      return exitCategory;
    }
  }
  final List<Entry> entries;
  try {
    entries = parseLedger(readLedger(options.option('file')));
  } on FileSystemException {
    err.writeln('ledger: cannot read ledger file');
    return exitLedger;
  } on LedgerException catch (e) {
    err.writeln('ledger: $e');
    return exitLedger;
  }
  switch (command) {
    case 'list':
      for (final entry in entries) {
        out.writeln(
          '${entry.date} ${entry.category.name} ${formatCents(entry.cents)}',
        );
      }
    case 'total':
      final sum = entries
          .where((entry) => entry.category == only)
          .fold(0, (total, entry) => total + entry.cents);
      out.writeln('${only!.name} ${formatCents(sum)}');
    default:
      for (final (category, cents) in totals(entries)) {
        out.writeln('${category.name} ${formatCents(cents)}');
      }
  }
  return exitOk;
}
