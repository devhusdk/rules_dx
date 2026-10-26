package Report;

use strict;
use warnings;

use Runfiles;
use Text::CSV;

my $UNITS_CSV = '_main/examples/perl-app/report/data/units.csv';

sub units_table {
    my $path = Runfiles->create()->rlocation($UNITS_CSV)
        or die "error: cannot locate $UNITS_CSV in runfiles\n";
    my $csv = Text::CSV->new({ binary => 1, auto_diag => 1 });
    open my $fh, '<:encoding(UTF-8)', $path or die "error: cannot open $path: $!\n";
    $csv->header($fh);
    my %table;
    while (my $row = $csv->getline_hr($fh)) {
        $table{ $row->{unit} } = $row->{units};
    }
    close $fh;
    return \%table;
}

sub quantity {
    my ($table, $count, $unit) = @_;
    die "error: $count is not a whole number\n" if $count !~ /\A[0-9]+\z/;
    my $per = $table->{ $unit // 'each' };
    die "error: unknown unit: $unit\n" if !defined $per;
    return $count * $per;
}

sub render {
    my (@rows) = @_;
    my $csv = Text::CSV->new({ binary => 1, eol => "\n" });
    my $out = '';
    for my $row ([ 'name', 'units' ], @rows) {
        $csv->combine(@{$row}) or die 'error: ' . $csv->error_diag . "\n";
        $out .= $csv->string;
    }
    return $out;
}

1;
