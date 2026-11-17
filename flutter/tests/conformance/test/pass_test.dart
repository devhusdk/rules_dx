import 'package:conformance/greet.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('passing_case', (tester) async {
    await tester.pumpWidget(const Greeting());
    expect(find.text('hello world'), findsOneWidget);
  });
}
