package Tool;
use strict;
use warnings;
use File::Spec;
use POSIX qw(floor);
use parent 'Exporter';
my $ok = eval { require Optional::Module; 1 };

=pod

use Not::In::Pod;

=cut

print <<'TEXT';
use Not::In::Heredoc;
TEXT
1;
