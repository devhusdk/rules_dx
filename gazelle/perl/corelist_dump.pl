# Prints the sorted list of modules shipped with the running Perl, one per
# line. With --check <file>, diffs that list against <file> instead and exits
# non-zero on drift.
use strict;
use warnings;
use Module::CoreList;

sub modules {
    my $release = $Module::CoreList::version{$]}
      or die "Module::CoreList does not know Perl $]\n";
    return sort keys %{$release};
}

if ( @ARGV && $ARGV[0] eq '--check' ) {
    my %want = map { $_ => 1 } modules();
    open my $fh, '<', $ARGV[1] or die "open $ARGV[1]: $!\n";
    my %have = map { chomp; $_ => 1 } grep { /\S/ } <$fh>;
    close $fh;
    my @missing = grep { !$have{$_} } sort keys %want;
    my @extra   = grep { !$want{$_} } sort keys %have;
    exit 0 unless @missing || @extra;
    warn
"corelist.txt drifted from Perl $]; regenerate with: bazel run //gazelle/perl:corelist_dump > gazelle/perl/corelist.txt\n";
    warn "missing: $_\n" for @missing;
    warn "extra: $_\n"   for @extra;

    if ( my $out = $ENV{TEST_UNDECLARED_OUTPUTS_DIR} ) {
        open my $w, '>', "$out/corelist.txt" or die "write: $!\n";
        print {$w} map { "$_\n" } modules();
        close $w;
    }
    exit 1;
}

print map { "$_\n" } modules();
