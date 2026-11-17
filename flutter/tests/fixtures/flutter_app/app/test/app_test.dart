import 'package:flutter/material.dart';
import 'package:flutter_app/app.dart';
import 'package:flutter_app/src/parts_model.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('adding a part updates the count and the list', (tester) async {
    final model = PartsModel(['Bolt', 'Washer']);
    await tester.pumpWidget(PartsApp(model: model, locale: const Locale('en')));
    expect(find.text('No items'), findsOneWidget);

    await tester.tap(find.byType(FloatingActionButton));
    await tester.pump();
    expect(find.text('One item'), findsOneWidget);
    expect(find.text('Bolt'), findsOneWidget);

    await tester.tap(find.byType(FloatingActionButton));
    await tester.pump();
    expect(find.text('2 items'), findsOneWidget);
  });

  testWidgets('uses the Danish translation', (tester) async {
    await tester.pumpWidget(PartsApp(model: PartsModel(['Bolt']), locale: const Locale('da')));
    expect(find.text('Ingen dele'), findsOneWidget);
  });

  test('totals group repeated parts', () {
    final model = PartsModel(['Bolt'])
      ..add()
      ..add();
    expect(model.totals, {'Bolt': 2});
  });
}
