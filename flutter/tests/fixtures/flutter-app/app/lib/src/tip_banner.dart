import 'package:flutter/material.dart';
import 'package:flutter_app/src/tips.g.dart';

class TipBanner extends StatelessWidget {
  const TipBanner({super.key, required this.index});

  final int index;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.all(8),
      child: Text(tips[index % tips.length], style: const TextStyle(fontFamily: 'DxSquare', package: 'flutter_app')),
    );
  }
}
