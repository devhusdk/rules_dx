use strict;
use warnings;

use Test::More tests => 2;
use Runfiles;

my $probe  = Runfiles->create()->rlocation( $ENV{PROBE_BIN} );
diag("$_=" . ( $ENV{$_} // '<unset>' ) )
  for qw(RUNFILES_DIR RUNFILES_MANIFEST_FILE RUNFILES_MANIFEST_ONLY TEST_SRCDIR);
diag("probe=$probe exists=" . ( -e $probe ? 1 : 0 ) );
my $mf = $ENV{RUNFILES_MANIFEST_FILE};
diag("manifest exists=" . ( defined $mf && -e $mf ? 1 : 0 ) );
if ( defined $mf && -e $mf ) {
    $ENV{RUNFILES_MANIFEST_FILE} = $mf =~ s{/}{\\}gr;
    delete $ENV{RUNFILES_DIR};
}
my $output = qx{"$probe"};

is( $?, 0, 'perl_binary launcher exits 0' );
like(
    $output,
    qr/\Aperl 5\.\d+\.\d+ at /,
    'perl_binary prints the toolchain version'
);
