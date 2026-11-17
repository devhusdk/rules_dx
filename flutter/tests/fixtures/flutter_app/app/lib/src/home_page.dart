import 'package:flutter/material.dart';
import 'package:flutter_app/l10n/app_localizations.dart';
import 'package:flutter_app/src/count_label.dart';
import 'package:flutter_app/src/parts_list.dart';
import 'package:flutter_app/src/parts_model.dart';
import 'package:flutter_app/src/tip_banner.dart';

class HomePage extends StatelessWidget {
  const HomePage({super.key, required this.model});

  final PartsModel model;

  @override
  Widget build(BuildContext context) {
    final l10n = AppLocalizations.of(context)!;
    return ListenableBuilder(
      listenable: model,
      builder: (context, _) => Scaffold(
        appBar: AppBar(title: Text(l10n.title)),
        body: Column(
          children: [
            TipBanner(index: model.count),
            CountLabel(count: model.count),
            Expanded(child: PartsList(parts: model.parts)),
          ],
        ),
        floatingActionButton: FloatingActionButton(
          onPressed: model.add,
          tooltip: l10n.addItem,
          child: const Icon(Icons.add),
        ),
      ),
    );
  }
}
