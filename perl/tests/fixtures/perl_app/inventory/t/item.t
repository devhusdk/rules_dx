use strict;
use warnings;

use Test::More tests => 4;
use Inventory::Item;

my $item = Inventory::Item->new( name => 'pears', units => 24 );
is( $item->name,  'pears', 'keeps the name' );
is( $item->units, 24,      'keeps the units' );

eval { Inventory::Item->new( name => 'pears', units => 0 ) };
is(
    $@,
    "error: pears needs a positive whole number of units\n",
    'rejects zero units'
);

eval { Inventory::Item->new( units => 1 ) };
is( $@, "error: an item needs a name\n", 'rejects a missing name' );
