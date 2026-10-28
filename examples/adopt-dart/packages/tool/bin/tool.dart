import 'package:args/args.dart';
import 'package:greet/greet.dart';

void main(List<String> arguments) {
  final parser = ArgParser()..addOption('name', defaultsTo: 'world');
  final results = parser.parse(arguments);
  print(greet(results['name'] as String));
}
