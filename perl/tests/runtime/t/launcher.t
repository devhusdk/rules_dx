use strict;
use warnings;

use Test::More tests => 2;
use Runfiles;

my $probe = Runfiles->create()->rlocation($ENV{PROBE_BIN});
my $output = qx{"$probe"};

is($?, 0, 'perl_binary launcher exits 0');
like($output, qr/\Aperl 5\.\d+\.\d+ at /, 'perl_binary prints the toolchain version');
