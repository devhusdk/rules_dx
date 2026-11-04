import 'package:dx_flutter_probe/main.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('shows the probe text', (tester) async {
    await tester.pumpWidget(const Probe());
    expect(find.text('dx flutter probe'), findsOneWidget);
  });
}
