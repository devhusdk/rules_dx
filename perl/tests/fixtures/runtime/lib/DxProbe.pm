package DxProbe;

use strict;
use warnings;

our $VERSION = '0.1.0';

sub interpreter {
    return $^X;
}

sub version {
    return sprintf '%vd', $^V;
}

sub read_text {
    my ($path) = @_;
    open my $fh, '<', $path or die "DxProbe: cannot open $path: $!\n";
    my $content = do { local $/; <$fh> };
    close $fh;
    return $content;
}

1;
