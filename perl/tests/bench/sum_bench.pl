use strict;
use warnings;
use DxBench qw(run_benchmarks);

my @numbers = ( 1 .. 1000 );

run_benchmarks(
    [
        sum_loop => sub {
            my $total = 0;
            $total += $_ for @numbers;
            return $total;
        },
        sum_join => sub { return eval join '+', @numbers },
    ],
    iterations => 20,
    rounds     => 3,
);
