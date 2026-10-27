use strict;
use warnings;
use utf8;

use Test::More tests => 7;
use Report;
use Text::CSV;

my $table = Report::units_table();
is_deeply($table, { each => 1, pair => 2, dozen => 12, gross => 144 }, 'units.csv loads from runfiles');
is(Report::quantity($table, 2, 'dozen'), 24, 'two dozen is 24');
is(Report::quantity($table, 3, undef), 3, 'a bare count means each');

eval { Report::quantity($table, 2, 'crate') };
is($@, "error: unknown unit: crate\n", 'unknown units fail');

is(Report::render([ 'pears', 24 ]), "name,units\npears,24\n", 'render writes a header and rows');
is(Report::render([ 'figs, dried', 2 ]), qq{name,units\n"figs, dried",2\n}, 'render quotes commas');
is(Text::CSV->backend, 'Text::CSV_PP', 'Text::CSV runs on the pure-Perl backend');
