use strict;
use warnings;
use JSON::PP ();
use Pod::Text ();

my %arg;
while (@ARGV) {
    my $flag = shift @ARGV;
    die "pod_dump: unknown argument '$flag'\n" unless $flag =~ /\A--(own|out)\z/;
    die "pod_dump: $flag needs a value\n" unless @ARGV;
    $arg{$1} = shift @ARGV;
}
die "pod_dump: --own and --out are required\n" unless $arg{own} && $arg{out};

sub slurp {
    my ($path) = @_;
    open my $fh, '<:raw', $path or die "pod_dump: cannot read $path: $!\n";
    local $/ = undef;
    my $text = <$fh>;
    close $fh;
    $text =~ s/\r\n/\n/g;
    return $text;
}

sub render {
    my ($body) = @_;
    return '' unless $body =~ /\S/;
    my $out = '';
    my $parser = Pod::Text->new( width => 100, indent => 0 );
    $parser->output_string( \$out );
    $parser->parse_string_document("=pod\n\n$body\n\n=cut\n");
    $out =~ s/\s+\z//;
    return $out;
}

sub pod_sections {
    my ($text) = @_;
    my %section;
    my ( $name, @body );
    my $in_pod = 0;
    my $flush = sub {
        $section{$name} = join( "\n", @body ) if defined $name && !exists $section{$name};
        ( $name, @body ) = ();
    };
    for my $line ( split /\n/, $text, -1 ) {
        if ( $line =~ /\A=cut\b/ ) {
            $in_pod = 0;
            next;
        }
        if ( $line =~ /\A=[a-zA-Z]/ ) {
            $in_pod = 1;
        }
        next unless $in_pod;
        if ( $line =~ /\A=(?:head[1-4]|item)\s+(.*?)\s*\z/ ) {
            $flush->();
            $name = $1;
            next;
        }
        push @body, $line;
    }
    $flush->();
    return \%section;
}

sub heading_name {
    my ($heading) = @_;
    $heading =~ s/[A-Z]<([^<>]*)>/$1/g;
    $heading =~ s/\A\*\s*//;
    return $1 if $heading =~ /(\w+)\s*(?:\(.*)?\z/ && $heading !~ /\s+-\s+/;
    return '';
}

my @objects;
open my $own, '<', $arg{own} or die "pod_dump: cannot read $arg{own}: $!\n";
my @files = map { chomp; [ split /\t/, $_, 3 ] } grep { /\S/ } <$own>;
close $own;

for my $entry (@files) {
    my ( $path, $repo, $generated ) = @$entry;
    my $text     = slurp($path);
    my $sections = pod_sections($text);
    my %documented;
    for my $heading ( keys %$sections ) {
        my $name = heading_name($heading);
        $documented{$name} = $sections->{$heading} if length $name && !exists $documented{$name};
    }
    my $current = '';
    my $in_pod  = 0;
    my $line_no = 0;
    for my $line ( split /\n/, $text, -1 ) {
        $line_no++;
        if ( $line =~ /\A=cut\b/ ) { $in_pod = 0; next }
        if ( $line =~ /\A=[a-zA-Z]/ ) { $in_pod = 1 }
        next if $in_pod;
        last if $line =~ /\A__(?:END|DATA)__\b/;
        if ( $line =~ /\A\s*package\s+([A-Za-z_]\w*(?:::\w+)*)\s*(?:[;{]|\z)/ ) {
            $current = $1;
            my $name_doc = '';
            for my $heading ( keys %$sections ) {
                next unless $heading =~ /\A(?:NAME|DESCRIPTION)\z/;
                $name_doc .= ( length $name_doc ? "\n\n" : '' ) . $sections->{$heading};
            }
            push @objects, {
                type       => 'package',
                path       => $current,
                name       => $current,
                namespace  => '',
                signature  => "package $current",
                docstring  => render($name_doc),
                file       => $repo,
                generated  => $generated ? JSON::PP::true : JSON::PP::false,
                line       => $line_no,
                visibility => 'public',
            };
            next;
        }
        if ( $current ne '' && $line =~ /\A\s*sub\s+([A-Za-z_]\w*)\s*(\([^)]*\))?\s*(?:\{|;|\z)/ ) {
            my ( $sub, $proto ) = ( $1, $2 // '' );
            push @objects, {
                type       => 'sub',
                path       => "${current}::$sub",
                name       => $sub,
                namespace  => $current,
                signature  => "sub $sub$proto",
                docstring  => render( $documented{$sub} // '' ),
                file       => $repo,
                generated  => $generated ? JSON::PP::true : JSON::PP::false,
                line       => $line_no,
                visibility => $sub =~ /\A_/ ? 'private' : 'public',
            };
        }
    }
}

open my $out, '>:raw', $arg{out} or die "pod_dump: cannot write $arg{out}: $!\n";
print {$out} JSON::PP->new->canonical->utf8->pretty->encode(
    {
        format  => 1,
        perl    => sprintf( '%vd', $^V ),
        objects => \@objects,
    }
);
close $out;
