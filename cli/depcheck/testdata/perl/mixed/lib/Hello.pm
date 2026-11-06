package Hello;
use Moo;
use Try::Tiny;

sub hello { try { "hello" } catch { "error" } }

1;
