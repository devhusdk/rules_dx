import 'package:build/build.dart';
import 'package:rules_dart_ext/worker_entry.dart';

class _ModelBuilder implements Builder {
  @override
  final buildExtensions = const {
    '.dart': ['.g.dart'],
  };

  @override
  Future<void> build(BuildStep buildStep) => buildStep.writeAsString(
    buildStep.inputId.changeExtension('.g.dart'),
    "const generatedValue = 'generated';\n",
  );
}

Future<void> main(List<String> args) =>
    shimMain(args, (BuilderOptions _) => _ModelBuilder());
