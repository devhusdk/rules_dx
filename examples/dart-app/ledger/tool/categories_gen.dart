import 'dart:io';

void main(List<String> args) {
  String? input;
  String? output;
  for (var i = 0; i + 1 < args.length; i++) {
    if (args[i] == '--input') input = args[i + 1];
    if (args[i] == '--output') output = args[i + 1];
  }
  if (input == null || output == null) {
    stderr.writeln('categories_gen: need --input and --output');
    exit(64);
  }
  final source = File(input).readAsStringSync();
  final start = source.indexOf("'''");
  final end = source.lastIndexOf("'''");
  if (start < 0 || end <= start) {
    stderr.writeln('categories_gen: $input has no category block');
    exit(65);
  }
  final names = source
      .substring(start + 3, end)
      .split('\n')
      .map((line) => line.trim())
      .where((line) => line.isNotEmpty)
      .toList();
  final valid = RegExp(r'^[a-z][a-z_]*$');
  for (final name in names) {
    if (!valid.hasMatch(name)) {
      stderr.writeln('categories_gen: bad category name "$name"');
      exit(65);
    }
  }
  final out = StringBuffer()
    ..writeln('// Generated from categories.dart. Do not edit.')
    ..writeln()
    ..writeln('/// A spending category.')
    ..writeln('enum Category {');
  for (final name in names) {
    out.writeln('  $name,');
  }
  out
    ..writeln('  ;')
    ..writeln()
    ..writeln('  /// Returns the category called [name], or null.')
    ..writeln('  static Category? named(String name) => values.asNameMap()[name];')
    ..writeln('}');
  File(output).writeAsStringSync(out.toString());
}
