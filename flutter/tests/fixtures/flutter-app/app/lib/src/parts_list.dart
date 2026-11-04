import 'package:flutter/material.dart';

class PartsList extends StatelessWidget {
  const PartsList({super.key, required this.parts});

  final List<String> parts;

  @override
  Widget build(BuildContext context) {
    return ListView.builder(
      itemCount: parts.length,
      itemBuilder: (context, index) => ListTile(title: Text(parts[index])),
    );
  }
}
