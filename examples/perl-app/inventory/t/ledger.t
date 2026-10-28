use strict;
use warnings;

use Test::More tests => 5;
use Inventory::Item;
use Inventory::Ledger;

my $ledger = Inventory::Ledger->new( limit => 30 );
is( $ledger->add( Inventory::Item->new( name => 'pears', units => 24 ) ),
    24, 'add returns the new total' );
is( $ledger->count('pears'), 24, 'count reads the total' );
is( $ledger->count('figs'),  0,  'unknown items count zero' );

eval { $ledger->add( Inventory::Item->new( name => 'pears', units => 7 ) ) };
is( $@, "error: pears would exceed the limit of 30\n", 'the limit holds' );

$ledger->add( Inventory::Item->new( name => 'apples', units => 3 ) );
is_deeply(
    [ $ledger->snapshot ],
    [ [ 'apples', 3 ], [ 'pears', 24 ] ],
    'snapshot sorts by name'
);
