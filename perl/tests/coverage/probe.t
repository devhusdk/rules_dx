use strict;
use warnings;

use File::Basename qw(dirname);
use File::Spec;
use Test::More tests => 3;

my $sf = 'perl/tests/coverage/hello.pl';

sub slurp {
    my ($path) = @_;
    open my $fh, '<', $path or return q{};
    local $/ = undef;
    my $text = <$fh>;
    close $fh or return q{};
    return $text;
}

if ( !$ENV{COVERAGE} ) {
    ok( !$INC{'DxLcov.pm'}, 'the collector stays unloaded outside coverage' );
    ok( 1,                  'nothing else to check' );
    ok( 1,                  'nothing else to check' );
    exit 0;
}

my $tmp      = $ENV{TEST_TMPDIR};
my $manifest = File::Spec->catfile( $tmp, 'manifest.txt' );
my $out      = File::Spec->catfile( $tmp, 'child.dat' );
open my $fh, '>', $manifest or die "manifest: $!";
print {$fh} "$sf\n";
close $fh or die "manifest: $!";

my $child =
  File::Spec->rel2abs( File::Spec->catfile( dirname(__FILE__), 'hello.pl' ) );
{
    local $ENV{COVERAGE_MANIFEST}    = $manifest;
    local $ENV{COVERAGE_OUTPUT_FILE} = $out;
    is( system( $^X, $child ), 0, 'the child run passes' );
}

my $lcov = slurp($out);
like( $lcov, qr{^SF:\Q$sf\E$}m, 'LCOV names the script by its workspace path' );
like( $lcov, qr{^DA:6,1$}m,     'the greet body ran once' );
