package Threads::Thread;
use strict;
use warnings;
use v5.16;

use YAML::Tiny;
use Digest::MD5 qw(md5_hex);
use Threads::Section qw(get_section set_section append_to_section has_section);

# Valid statuses
our @ACTIVE_STATUSES = qw(idea planning active blocked paused);
our @TERMINAL_STATUSES = qw(resolved superseded deferred rejected);
our @ALL_STATUSES = (@ACTIVE_STATUSES, @TERMINAL_STATUSES);

# Precompiled regex patterns for notes/todos with hash markers
# Pattern: "- content  <!-- hash -->" for notes
# Pattern: "- [ ] content  <!-- hash -->" or "- [x] content  <!-- hash -->" for todos
sub _note_edit_re {
    my ($hash) = @_;
    return qr/^(- ).*?(  <!-- $hash -->)$/m;
}

sub _note_remove_re {
    my ($hash) = @_;
    return qr/^- .*?  <!-- $hash -->\n/m;
}

sub _todo_check_re {
    my ($hash) = @_;
    return qr/^(- )\[ \](.*<!-- $hash -->)/m;
}

sub _todo_uncheck_re {
    my ($hash) = @_;
    return qr/^(- )\[x\](.*<!-- $hash -->)/m;
}

sub _todo_remove_re {
    my ($hash) = @_;
    return qr/^- \[[ x]\] .*?  <!-- $hash -->\n/m;
}

# Precompiled patterns for log date handling
our $LOG_DATE_RE = qr/^### (\d{4}-\d{2}-\d{2})$/m;

# Create new thread object from file (full YAML parse)
sub new_from_file {
    my ($class, $path) = @_;

    open my $fh, '<:encoding(UTF-8)', $path
        or die "Cannot read thread file: $path: $!\n";
    local $/;
    my $content = <$fh>;
    close $fh;

    my ($meta, $body) = _parse_frontmatter($content);
    die "Invalid thread file (no frontmatter): $path\n" unless $meta;

    return bless {
        path   => $path,
        id     => $meta->{id},
        name   => $meta->{name},
        desc   => $meta->{desc} // '',
        status => $meta->{status},
        _body  => $body,
    }, $class;
}

# Create thread object with lazy frontmatter extraction (fast path for listing)
# Only reads frontmatter fields via regex, skips full YAML parse
sub new_from_file_lazy {
    my ($class, $path) = @_;

    open my $fh, '<:encoding(UTF-8)', $path
        or die "Cannot read thread file: $path: $!\n";

    # Read only the frontmatter section
    my $header = '';
    my $in_fm = 0;
    while (<$fh>) {
        if (/^---\s*$/) {
            if ($in_fm) {
                last;  # closing delimiter
            }
            $in_fm = 1;
            next;
        }
        $header .= $_ if $in_fm;
    }
    close $fh;

    die "Invalid thread file (no frontmatter): $path\n" unless $header;

    # Extract fields via regex (no YAML parse)
    my %meta;
    $meta{id}     = $1 if $header =~ /^id:\s*(\S+)/m;
    $meta{name}   = $1 if $header =~ /^name:\s*["']?(.+?)["']?\s*$/m;
    $meta{desc}   = $1 if $header =~ /^desc:\s*["']?(.+?)["']?\s*$/m;
    $meta{status} = $1 if $header =~ /^status:\s*(\S.*?)\s*$/m;

    return bless {
        path   => $path,
        id     => $meta{id},
        name   => $meta{name},
        desc   => $meta{desc} // '',
        status => $meta{status},
        _lazy  => 1,  # marker for lazy-loaded (no body available)
    }, $class;
}

# Create new thread (not yet saved)
sub new {
    my ($class, %args) = @_;

    my $id = $args{id} // _generate_id();

    return bless {
        id     => $id,
        name   => $args{name} // die("Thread name required\n"),
        desc   => $args{desc} // '',
        status => $args{status} // 'idea',
        _body  => _initial_body(),
    }, $class;
}

# Accessors
sub id     { $_[0]->{id} }
sub name   { $_[0]->{name} }
sub desc   { $_[0]->{desc} }
sub status { $_[0]->{status} }
sub path   { $_[0]->{path} }

# Status without reason (e.g., "resolved (duplicate)" -> "resolved")
# Can be called as method ($thread->base_status) or function (base_status($str))
sub base_status {
    my ($arg) = @_;
    my $s = ref($arg) ? $arg->{status} : $arg;
    $s =~ s/\s.*//;
    return $s;
}

sub is_terminal {
    my ($self) = @_;
    my $base = $self->base_status;
    return grep { $_ eq $base } @TERMINAL_STATUSES;
}

# Validate status string (can be called as function or method)
# Accepts full status with parenthetical suffix like "blocked (waiting on review)"
sub is_valid_status {
    my ($status) = @_;
    my $base = base_status($status);
    return grep { $_ eq $base } @ALL_STATUSES;
}

# Setters
sub set_name   { $_[0]->{name} = $_[1] }
sub set_desc   { $_[0]->{desc} = $_[1] }
sub set_status { $_[0]->{status} = $_[1] }

# Get raw content (frontmatter + body)
sub content {
    my ($self) = @_;
    return $self->_to_frontmatter . $self->{_body};
}

# Body section operations
sub body {
    my ($self) = @_;
    return get_section($self->{_body}, 'Body');
}

sub set_body {
    my ($self, $text) = @_;
    $self->{_body} = set_section($self->{_body}, 'Body', $text);
}

sub append_body {
    my ($self, $text) = @_;
    $self->{_body} = append_to_section($self->{_body}, 'Body', $text);
}

# Note operations
sub add_note {
    my ($self, $text) = @_;
    my $hash = _generate_hash($text);
    my $line = "- $text  <!-- $hash -->\n";
    $self->{_body} = append_to_section($self->{_body}, 'Notes', $line);
    return $hash;
}

sub edit_note {
    my ($self, $hash, $new_text) = @_;
    my $re = _note_edit_re($hash);
    $self->{_body} =~ s/$re/$1$new_text$2/
        or die "Note $hash not found\n";
}

sub remove_note {
    my ($self, $hash) = @_;
    my $re = _note_remove_re($hash);
    $self->{_body} =~ s/$re//
        or die "Note $hash not found\n";
}

# Todo operations
sub add_todo {
    my ($self, $text) = @_;
    my $hash = _generate_hash($text);
    my $line = "- [ ] $text  <!-- $hash -->\n";
    $self->{_body} = append_to_section($self->{_body}, 'Todo', $line);
    return $hash;
}

sub check_todo {
    my ($self, $hash) = @_;
    my $re = _todo_check_re($hash);
    $self->{_body} =~ s/$re/$1\[x\]$2/
        or die "Todo $hash not found or already checked\n";
}

sub uncheck_todo {
    my ($self, $hash) = @_;
    my $re = _todo_uncheck_re($hash);
    $self->{_body} =~ s/$re/$1\[ \]$2/
        or die "Todo $hash not found or already unchecked\n";
}

sub remove_todo {
    my ($self, $hash) = @_;
    my $re = _todo_remove_re($hash);
    $self->{_body} =~ s/$re//
        or die "Todo $hash not found\n";
}

# Log operations
sub add_log_entry {
    my ($self, $entry) = @_;

    my ($sec, $min, $hour, $mday, $mon, $year) = localtime;
    my $date = sprintf "%04d-%02d-%02d", $year + 1900, $mon + 1, $mday;
    my $time = sprintf "%02d:%02d", $hour, $min;

    my $log = get_section($self->{_body}, 'Log');
    my $formatted = "- **$time** $entry\n";

    # Check if today's date header exists
    if ($log =~ /^### $date$/m) {
        # Insert after date header (and any existing entries for that day)
        $self->{_body} =~ s/(### $date\n(?:.*\n)*?)(\n### |\n## |\z)/$1$formatted\n$2/s;
    } else {
        # Add new date header
        my $new_entry = "### $date\n\n$formatted";
        $self->{_body} = append_to_section($self->{_body}, 'Log', "\n$new_entry");
    }
}

# Save to file
sub save {
    my ($self, $path) = @_;
    $path //= $self->{path};
    die "No path specified for save\n" unless $path;

    open my $fh, '>:encoding(UTF-8)', $path
        or die "Cannot write thread file: $path: $!\n";
    print $fh $self->content;
    close $fh;

    $self->{path} = $path;
}

# Private methods

sub _parse_frontmatter {
    my ($content) = @_;

    return (undef, $content) unless $content =~ /\A---\n(.+?)\n---\n(.*)/s;
    my ($yaml_str, $body) = ($1, $2);

    # Pre-check for common malformed YAML that YAML::Tiny is lenient about
    # Detect unclosed brackets/braces in values
    for my $line (split /\n/, $yaml_str) {
        if ($line =~ /:\s*\[[^\]]*$/ || $line =~ /:\s*\{[^\}]*$/) {
            # Value starts with [ or { but doesn't close on same line
            return (undef, $content);
        }
    }

    # YAML::Tiny expects document start marker
    my $yaml = YAML::Tiny->read_string("---\n$yaml_str\n");

    # Check for parse errors (note: errstr is deprecated but still works)
    if (!$yaml || !$yaml->[0]) {
        return (undef, $content);
    }

    # Verify we got a hash (not an array or scalar from malformed YAML)
    my $data = $yaml->[0];
    unless (ref($data) eq 'HASH') {
        return (undef, $content);
    }

    return ($data, $body);
}

sub _to_frontmatter {
    my ($self) = @_;

    # Build YAML manually to control field order
    my @lines = ('---');
    push @lines, "id: $self->{id}";
    push @lines, "name: " . _yaml_quote($self->{name});
    push @lines, "desc: " . _yaml_quote($self->{desc}) if length $self->{desc};
    push @lines, "status: $self->{status}";
    push @lines, '---';

    return join("\n", @lines) . "\n";
}

sub _yaml_quote {
    my ($str) = @_;
    # Quote if contains special chars
    if ($str =~ /[:#\[\]{}|>&*!?,]/ || $str =~ /^['"]/ || $str =~ /^\s|\s$/) {
        $str =~ s/"/\\"/g;
        return qq{"$str"};
    }
    return $str;
}

sub _initial_body {
    return <<'END';

## Body

## Notes

## Todo

## Log

END
}

sub _generate_id {
    # 6 hex chars from /dev/urandom
    if (open my $fh, '<', '/dev/urandom') {
        read $fh, my $bytes, 3;
        close $fh;
        return unpack('H6', $bytes);
    }
    # Fallback: use time + pid
    return substr(md5_hex(time() . $$), 0, 6);
}

sub _generate_hash {
    my ($text) = @_;
    my $input = $text . time() . $$ . rand();
    return substr(md5_hex($input), 0, 4);
}

1;
