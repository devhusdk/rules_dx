package DxLcov;

use strict;
use warnings;

our $VERSION = '1.0';

our %HITS;

BEGIN {
    # 0x02 calls DB::DB per statement, 0x10 and 0x100 keep the source line
    # arrays that mark which lines can run.
    $^P         = 0x02 | 0x10 | 0x100;
    $DB::single = 1;
}

sub DB::DB {
    my ( undef, $file, $line ) = caller;
    $HITS{$file}{$line}++;
    return;
}

sub _manifest {
    my $path = $ENV{COVERAGE_MANIFEST};
    return if !defined $path;
    open my $fh, '<', $path or return;
    my %paths;
    while ( my $line = <$fh> ) {
        $line =~ s/\s+\z//smx;
        $paths{$line} = 1 if length $line;
    }
    close $fh or return;
    return \%paths;
}

sub _owned_path {
    my ( $file, $paths ) = @_;
    ( my $path = $file ) =~ tr{\\}{/};
    $path =~ s{\A\./}{}smx;
    while ( !$paths->{$path} ) {
        return if $path !~ s{\A[^/]*/}{}smx;
    }
    return $path;
}

sub _record {
    my ( $file, $path ) = @_;
    no strict 'refs';    ## no critic (ProhibitNoStrict)
    my @source = @{"main::_<$file"};
    my $hits   = $HITS{$file};
    my %lines  = map { $_ => 0 } grep { _runs( $source[$_] ) } 1 .. $#source;
    $lines{$_} = $hits->{$_} for keys %{$hits};
    my @rows = map  { "DA:$_,$lines{$_}" } sort { $a <=> $b } keys %lines;
    my $hit  = grep { $lines{$_} > 0 } keys %lines;
    return join "\n", "SF:$path", @rows, 'LF:' . scalar(@rows), "LH:$hit",
      'end_of_record', q{};
}

sub _runs {
    my ($value) = @_;
    no warnings 'numeric';
    return defined $value && $value != 0;
}

sub write_report {
    my $out   = $ENV{COVERAGE_OUTPUT_FILE};
    my $paths = _manifest();
    return if !defined $out || !defined $paths;
    my $report = q{};
    for my $file ( sort keys %HITS ) {
        my $path = _owned_path( $file, $paths );
        $report .= _record( $file, $path ) if defined $path;
    }
    return if !length $report;
    open my $fh, '>', $out or return;
    print {$fh} $report;
    close $fh or return;
    return 1;
}

END {
    $^P = 0;
    write_report();
}

1;
