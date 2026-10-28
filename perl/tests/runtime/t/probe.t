use strict;
use warnings;

use Test::More tests => 5;
use DxProbe;
use Runfiles;

my $runfiles = Runfiles->create();
my $perl     = DxProbe::interpreter();

unlike( $perl, qr{\A/(?:usr|bin|opt)/},
    "interpreter is not a host perl: $perl" );
like(
    $perl,
    qr{perl_(?:linux|darwin|windows)_(?:amd64|arm64|x86_64)},
    "interpreter comes from the rules_perl toolchain: $perl"
);

SKIP: {
    skip 'Strawberry perl pins its own version on Windows', 1
      if $^O eq 'MSWin32';
    like( DxProbe::version(), qr/\A5\.40\./, 'runtime is perl 5.40' );
}

my $module = $INC{'DxProbe.pm'};
my $status = system $perl, '-c', $module;
is( $status, 0, "perl -c compiles $module" );

my $data = $runfiles->rlocation( $ENV{DATA_FILE} );
is(
    DxProbe::read_text($data),
    "Hello from dx perl\n",
    'data.txt resolves through runfiles'
);
