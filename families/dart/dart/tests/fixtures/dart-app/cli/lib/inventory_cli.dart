import 'package:args/args.dart';
import 'package:ledger/ledger.dart';
import 'package:model/model.dart';

/// Exit code for a successful run.
const int exitOk = 0;

/// Exit code for bad arguments.
const int exitUsage = 2;

/// Exit code for an item that is not in the ledger.
const int exitUnknownItem = 4;

/// Runs the CLI over the CSV at [csvPath]. Output goes through [out] and [err].
int run(List<String> arguments, String csvPath, void Function(String) out, void Function(String) err) {
  final ArgResults results;
  try {
    results = ArgParser().parse(arguments);
  } on FormatException catch (e) {
    err('usage: inventory [get NAME]: ${e.message}');
    return exitUsage;
  }
  final rest = results.rest;
  final items = loadItems(csvPath);
  if (rest.isEmpty) {
    for (final item in items) {
      out('${item.name} ${item.qty} ${item.priceCents}');
    }
    out('total ${totalCents(items)}');
    out('schema $schemaVersion');
    return exitOk;
  }
  if (rest.length != 2 || rest.first != 'get') {
    err('usage: inventory [get NAME]');
    return exitUsage;
  }
  for (final item in items) {
    if (item.name == rest[1]) {
      out('${item.name} ${item.qty} ${item.priceCents}');
      return exitOk;
    }
  }
  err('unknown item: ${rest[1]}');
  return exitUnknownItem;
}
