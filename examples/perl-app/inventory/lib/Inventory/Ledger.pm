package Inventory::Ledger;

use strict;
use warnings;

sub new {
    my ($class, %args) = @_;
    my $limit = $args{limit} // 60;
    die "error: the limit must be a positive whole number\n" if $limit !~ /\A[1-9][0-9]*\z/;
    return bless { limit => 0 + $limit, stock => {} }, $class;
}

sub limit {
    my ($self) = @_;
    return $self->{limit};
}

sub add {
    my ($self, $item) = @_;
    my $total = $self->count($item->name) + $item->units;
    die sprintf "error: %s would exceed the limit of %d\n", $item->name, $self->{limit}
        if $total > $self->{limit};
    $self->{stock}{ $item->name } = $total;
    return $total;
}

sub count {
    my ($self, $name) = @_;
    return $self->{stock}{$name} // 0;
}

sub snapshot {
    my ($self) = @_;
    return map { [ $_, $self->{stock}{$_} ] } sort keys %{ $self->{stock} };
}

1;
