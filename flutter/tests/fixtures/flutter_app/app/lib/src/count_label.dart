import 'package:flutter/material.dart';
import 'package:flutter_app/l10n/app_localizations.dart';

class CountLabel extends StatelessWidget {
  const CountLabel({super.key, required this.count});

  final int count;

  @override
  Widget build(BuildContext context) {
    return Text(AppLocalizations.of(context)!.count(count), key: const Key('count'));
  }
}
