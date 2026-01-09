package Threads::Section;
use strict;
use warnings;
use v5.16;

use Exporter 'import';

our @EXPORT_OK = qw(
    get_section
    set_section
    append_to_section
    has_section
);

# Precompiled pattern generators for section operations
sub _section_header_re {
    my ($section) = @_;
    return qr/^##\s*\Q$section\E\s*$/m;
}

sub _section_content_re {
    my ($section) = @_;
    return qr/^##\s*\Q$section\E\s*\n(.*?)(?=^##\s|\z)/ms;
}

sub _section_replace_re {
    my ($section) = @_;
    return qr/(^##\s*\Q$section\E\s*\n).*?(?=^##\s|\z)/ms;
}

# Get content of a markdown section (## Name)
sub get_section {
    my ($content, $section) = @_;
    my $re = _section_content_re($section);

    # Match section header and capture until next section or end
    if ($content =~ $re) {
        return $1;
    }
    return '';
}

# Check if a section exists
sub has_section {
    my ($content, $section) = @_;
    my $re = _section_header_re($section);
    return $content =~ $re;
}

# Set content of a markdown section (replaces existing or creates new)
sub set_section {
    my ($content, $section, $new_text) = @_;

    # Ensure trailing newline
    $new_text =~ s/\n*$/\n/ if length $new_text;

    if (has_section($content, $section)) {
        # Replace existing section content (header + content until next section or EOF)
        my $re = _section_replace_re($section);
        $content =~ s/$re/$1$new_text/;
    } else {
        # Append new section at end
        $content =~ s/\n*$/\n/;  # Ensure single trailing newline
        $content .= "\n## $section\n$new_text";
    }

    return $content;
}

# Append to a section (creates section if needed)
sub append_to_section {
    my ($content, $section, $text) = @_;

    if (has_section($content, $section)) {
        my $existing = get_section($content, $section);
        return set_section($content, $section, $existing . $text);
    } else {
        return set_section($content, $section, $text);
    }
}

1;
