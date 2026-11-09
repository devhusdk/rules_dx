use strict;
use warnings;
use JSON::PP qw(decode_json);
use Test::More tests => 3;

my $dir = $ENV{TEST_TMPDIR} // '.';
$ENV{TEST_UNDECLARED_OUTPUTS_DIR} = $dir;
use DxBench qw(run_benchmarks);

is( run_benchmarks( [ noop => sub { 1 } ], iterations => 2, rounds => 2 ), 1, 'one case' );
open my $fh, '<', "$dir/dx-bench.json" or die $!;
my $doc = decode_json( do { local $/; <$fh> } );
is( $doc->{schema}, 'dx-bench/v1', 'schema' );
is( $doc->{results}[0]{name}, 'noop', 'name' );
