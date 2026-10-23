import 'package:ledger_cli/cli.dart';
import 'package:test/test.dart';

const _ledger = '''
date,category,cents
2026-01-02,rent,120000
2026-01-03,food,2350
2026-01-09,food,1875
''';

(int, String, String) _run(List<String> args, {String text = _ledger}) {
  final out = StringBuffer();
  final err = StringBuffer();
  final code = run(args, out: out, err: err, readLedger: (_) => text);
  return (code, '$out', '$err');
}

void main() {
  test('totals sorts by amount', () {
    expect(_run(['totals']), (exitOk, 'rent 1200.00\nfood 42.25\n', ''));
  });

  test('total sums one category', () {
    expect(_run(['total', 'food']), (exitOk, 'food 42.25\n', ''));
  });

  test('list keeps file order', () {
    final (code, out, _) = _run(['list']);
    expect(code, exitOk);
    expect(out.split('\n').first, '2026-01-02 rent 1200.00');
  });

  test('version defaults to dev outside the binary', () {
    expect(_run(['--version']), (exitOk, 'ledger dev\n', ''));
  });

  test('unknown category exits 4 before reading the ledger', () {
    final out = StringBuffer();
    final err = StringBuffer();
    final code = run(
      ['total', 'toys'],
      out: out,
      err: err,
      readLedger: (_) => throw StateError('ledger must not be read'),
    );
    expect(code, exitCategory);
    expect('$err', 'ledger: no category toys\n');
  });

  test('usage errors exit 2', () {
    expect(_run([]).$1, exitUsage);
    expect(_run(['sell']).$1, exitUsage);
    expect(_run(['total']).$1, exitUsage);
    expect(_run(['--colour']).$1, exitUsage);
  });

  test('malformed ledger exits 3 with its reason', () {
    expect(_run(['totals'], text: 'date,category,cents\n'), (
      exitLedger,
      '',
      'ledger: ledger has no entries\n',
    ));
  });
}
