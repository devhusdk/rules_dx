#!/usr/bin/env perl
# Prints the CPANSA::DB version, a newline, then its advisories keyed by
# distribution as JSON. advisory_prep turns that into the cpan snapshot.
use strict;
use warnings;
use CPANSA::DB;
use JSON::PP;

print $CPANSA::DB::VERSION, qq{\n},
    JSON::PP->new->canonical->encode(CPANSA::DB->db->{dists});
