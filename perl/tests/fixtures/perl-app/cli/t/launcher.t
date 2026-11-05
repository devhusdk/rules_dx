use strict;
use warnings;

use Test::More tests => 4;
use Runfiles;

my $stockctl = Runfiles->create()->rlocation( $ENV{STOCKCTL} );
my $limit    = $ENV{WANT_LIMIT};

my $mf = $ENV{RUNFILES_MANIFEST_FILE};
if ( defined $mf && -e $mf ) {
    $ENV{RUNFILES_MANIFEST_FILE} = $mf =~ s{/}{\\}gr;
    delete $ENV{RUNFILES_DIR};
}

sub stockctl {
    my (@args) = @_;
    my $q      = $^O eq 'MSWin32' ? '"' : "'";
    my $quoted = join ' ', map { "$q$_$q" } @args;
    my $out    = qx{"$stockctl" $quoted 2>&1};
    return ( $? >> 8, $out );
}

is_deeply(
    [ stockctl( 'add 2 dozen pears', 'count pears' ) ],
    [ 0, "24\n24\nname,units\npears,24\n" ],
    'add and count'
);
is_deeply(
    [ stockctl() ],
    [ 0, "name,units\n" ],
    'no commands prints an empty report'
);
is_deeply(
    [ stockctl('add 13 dozen pears') ],
    [ 1, "error: pears would exceed the limit of $limit\n" ],
    "the limit is $limit"
);
is_deeply(
    [ stockctl('sell pears') ],
    [ 1, "error: unknown command: sell pears\n" ],
    'unknown commands fail'
);
