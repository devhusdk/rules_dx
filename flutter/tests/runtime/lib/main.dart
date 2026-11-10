import 'package:flutter/widgets.dart';

void main() {
  runApp(const Probe());
}

class Probe extends StatelessWidget {
  const Probe({super.key});

  @override
  Widget build(BuildContext context) {
    return const Directionality(
      textDirection: TextDirection.ltr,
      child: Text('dx flutter probe'),
    );
  }
}
