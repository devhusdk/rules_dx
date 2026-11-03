import 'dart:io';

void main(List<String> args) {
  final outputs = <String>[];
  for (var i = 0; i < args.length - 1; i++) {
    if (args[i] == '--output') outputs.add(args[i + 1]);
  }
  File(outputs.single).writeAsStringSync("const generatedValue = 'generated';\n");
}
