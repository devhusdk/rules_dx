import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_app/app.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  runApp(PartsApp(model: await loadModel(rootBundle)));
}
