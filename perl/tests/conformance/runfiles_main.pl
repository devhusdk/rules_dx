use strict;
use warnings;

use Runfiles;

my $path = Runfiles->create()->rlocation( $ARGV[0] );
open my $fh, '<:raw', $path or die "cannot open $ARGV[0]: $!";
local $/;
print <$fh>;
