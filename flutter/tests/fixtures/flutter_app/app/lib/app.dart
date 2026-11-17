import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_app/l10n/app_localizations.dart';
import 'package:flutter_app/src/home_page.dart';
import 'package:flutter_app/src/parts_model.dart';
import 'package:flutter_localizations/flutter_localizations.dart';

Future<PartsModel> loadModel(AssetBundle bundle) async {
  final data = jsonDecode(await bundle.loadString('packages/flutter_app/assets/items.json')) as Map<String, Object?>;
  return PartsModel((data['items'] as List<Object?>).cast<String>());
}

class PartsApp extends StatelessWidget {
  const PartsApp({super.key, required this.model, this.locale});

  final PartsModel model;
  final Locale? locale;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      theme: ThemeData(fontFamily: 'DxSquare', package: 'flutter_app'),
      locale: locale,
      localizationsDelegates: const [
        AppLocalizations.delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
      ],
      supportedLocales: AppLocalizations.supportedLocales,
      home: HomePage(model: model),
    );
  }
}
