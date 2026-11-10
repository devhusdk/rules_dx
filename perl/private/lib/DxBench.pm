package DxBench;

use strict;
use warnings;
use Benchmark qw(timeit);
use Exporter qw(import);
use Time::HiRes qw(time);
use JSON::PP ();

our @EXPORT_OK = qw(run_benchmarks);

my $DEFAULT_ITERATIONS = 100;
my $DEFAULT_ROUNDS     = 5;

sub _measure {
    my ( $code, $iterations, $rounds ) = @_;
    my @times;
    for ( 1 .. $rounds ) {
        my $start = time;
        timeit( $iterations, $code );
        push @times, ( time - $start ) / $iterations;
    }
    my $mean = 0;
    $mean += $_ for @times;
    $mean /= @times;
    my $var = 0;
    $var += ( $_ - $mean )**2 for @times;
    my $stddev = @times > 1 ? sqrt( $var / ( @times - 1 ) ) : 0;
    return ( $mean, $stddev );
}

sub run_benchmarks {
    my ( $cases, %opts ) = @_;
    my $iterations = $opts{iterations} // $DEFAULT_ITERATIONS;
    my $rounds     = $opts{rounds}     // $DEFAULT_ROUNDS;
    my @results;
    while ( my ( $name, $code ) = splice @{$cases}, 0, 2 ) {
        my ( $mean, $stddev ) = _measure( $code, $iterations, $rounds );
        push @results,
          {
            name       => $name,
            unit       => 's',
            mean       => $mean + 0,
            stddev     => $stddev + 0,
            iterations => $iterations * $rounds,
          };
    }
    my $dir = $ENV{TEST_UNDECLARED_OUTPUTS_DIR};
    die "TEST_UNDECLARED_OUTPUTS_DIR is not set\n" unless defined $dir;
    open my $fh, '>', "$dir/dx-bench.json" or die "dx-bench.json: $!\n";
    print {$fh} JSON::PP->new->canonical->encode(
        { schema => 'dx-bench/v1', results => \@results } );
    close $fh or die "dx-bench.json: $!\n";
    return scalar @results;
}

1;
