import 'package:flutter/widgets.dart';

String greet(String name) => 'hello $name';

class Greeting extends StatelessWidget {
  const Greeting({super.key});

  @override
  Widget build(BuildContext context) {
    return Text(greet('world'), textDirection: TextDirection.ltr);
  }
}
