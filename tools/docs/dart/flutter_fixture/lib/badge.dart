import 'package:flutter/widgets.dart';

/// A small label that shows how many items are pending.
class CountBadge extends StatelessWidget {
  /// Creates a badge that shows [count].
  const CountBadge({super.key, required this.count});

  /// The number of pending items.
  final int count;

  @override
  Widget build(BuildContext context) {
    return Text('$count');
  }
}
