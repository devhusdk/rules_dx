use strict;
use warnings;

use Inventory::Item;
use Inventory::Ledger;
use Report;

binmode STDOUT, ':encoding(UTF-8)';
binmode STDERR, ':encoding(UTF-8)';

sub run {
    my (@commands) = @_;
    my $ledger = Inventory::Ledger->new(limit => $ENV{STOCK_LIMIT} // 60);
    my $table = Report::units_table();
    for my $command (@commands) {
        my @words = split ' ', $command;
        my $verb = shift(@words) // '';
        if ($verb eq 'add' && (@words == 2 || @words == 3)) {
            my $name = pop @words;
            my $units = Report::quantity($table, @words);
            print $ledger->add(Inventory::Item->new(name => $name, units => $units)), "\n";
        }
        elsif ($verb eq 'count' && @words == 1) {
            print $ledger->count($words[0]), "\n";
        }
        else {
            die "error: unknown command: $command\n";
        }
    }
    print Report::render($ledger->snapshot);
    return 0;
}

my @args = map { utf8::decode(my $arg = $_); $arg } @ARGV;
my $code = eval { run(@args) };
if (!defined $code) {
    print STDERR $@;
    exit 1;
}
exit $code;
