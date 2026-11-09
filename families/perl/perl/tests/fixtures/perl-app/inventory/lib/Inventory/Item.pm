package Inventory::Item;

use strict;
use warnings;

sub new {
    my ( $class, %args ) = @_;
    my $name  = $args{name}  // '';
    my $units = $args{units} // '';
    die "error: an item needs a name\n" if $name eq '';
    die "error: $name needs a positive whole number of units\n"
      if $units !~ /\A[1-9][0-9]*\z/;
    return bless { name => $name, units => 0 + $units }, $class;
}

sub name {
    my ($self) = @_;
    return $self->{name};
}

sub units {
    my ($self) = @_;
    return $self->{units};
}

1;
