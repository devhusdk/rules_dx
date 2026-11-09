package Geometry;

use strict;
use warnings;

our $VERSION = '1.0';

sub new {
    my ( $class, %args ) = @_;
    return bless {%args}, $class;
}

sub area {
    my ($self) = @_;
    return $self->{width} * $self->{height};
}

sub _scale {
    my ( $self, $factor ) = @_;
    return $factor;
}

1;

__END__

=head1 NAME

Geometry - Areas of simple shapes.

=head1 DESCRIPTION

Measures rectangles.

=head2 new

Builds a rectangle from C<width> and C<height>.

=head2 area

Returns the area as a number.

=cut
