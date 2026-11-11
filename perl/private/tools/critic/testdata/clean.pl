use strict;
use warnings;

our $VERSION = '1.0';

my $name = 'world';
my $text = <<"END";
hello $name
END
my @words = qw(a b c);
my $re = qr/a+b/;
print $text, "@words" =~ $re ? "ok\n" : "no\n";


