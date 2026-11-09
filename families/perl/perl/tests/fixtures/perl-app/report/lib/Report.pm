package Report;

use strict;
use warnings;

use Runfiles;
use Text::CSV;

my $UNITS_CSV = 'perl/tests/fixtures/perl-app/report/data/units.csv';
my @UNITS_REPOS = ( '_main', 'rules_dx_perl' );

sub units_table {
    my $runfiles = Runfiles->create();
    my ($path) = grep { defined && -e } map { $runfiles->rlocation("$_/$UNITS_CSV") } @UNITS_REPOS;
    die "error: cannot locate $UNITS_CSV in runfiles\n" unless defined $path;
    my $csv = Text::CSV->new( { binary => 1, auto_diag => 1 } );
    open my $fh, '<:encoding(UTF-8)', $path
      or die "error: cannot open $path: $!\n";
    $csv->header($fh);
    my %table;
    while ( my $row = $csv->getline_hr($fh) ) {
        $table{ $row->{unit} } = $row->{units};
    }
    close $fh;
    return \%table;
}

sub quantity {
    my ( $table, $count, $unit ) = @_;
    die "error: $count is not a whole number\n" if $count !~ /\A[0-9]+\z/;
    my $per = $table->{ $unit // 'each' };
    die "error: unknown unit: $unit\n" if !defined $per;
    return $count * $per;
}

sub render {
    my (@rows) = @_;
    my $csv    = Text::CSV->new( { binary => 1, eol => "\n" } );
    my $out    = '';
    for my $row ( [ 'name', 'units' ], @rows ) {
        $csv->combine( @{$row} ) or die 'error: ' . $csv->error_diag . "\n";
        $out .= $csv->string;
    }
    return $out;
}

1;
