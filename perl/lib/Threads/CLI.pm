package Threads::CLI;
use strict;
use warnings;
use v5.16;

use open ':std', ':encoding(UTF-8)';
use Cwd qw(abs_path);
use Getopt::Long qw(:config no_auto_abbrev);
use File::Basename qw(basename dirname);
use File::Path qw(make_path);
use File::Spec;

use Threads::Workspace qw(workspace_root infer_scope find_thread find_all_threads slugify);
use Threads::Thread;
use Threads::Git qw(git_commit git_commit_pending git_cmd git_capture);

# Main entry point
sub run {
    my ($class, @argv) = @_;

    # Handle --help and --version before command dispatch
    if (!@argv || $argv[0] eq '--help' || $argv[0] eq '-h') {
        return cmd_help();
    }
    if ($argv[0] eq '--version' || $argv[0] eq '-v') {
        say "threads (perl) 0.1.0";
        return 0;
    }

    my $cmd = shift @argv;
    my $method = "cmd_$cmd";
    $method =~ s/-/_/g;

    if (__PACKAGE__->can($method)) {
        my $result = eval { __PACKAGE__->$method(@argv) };
        if ($@) {
            my $msg = $@;
            chomp $msg;
            warn "$msg\n" if $msg && $msg !~ /^\s*$/;
            return 1;
        }
        return $result // 0;
    } else {
        warn "Unknown command: $cmd\n";
        return 1;
    }
}

# ============================================================================
# Help
# ============================================================================

sub cmd_help {
    print <<'HELP';
threads - Persistent topic tracking for LLM workflows

Usage: threads <command> [options]

Commands:
  list, ls [path]          List threads (default: current scope)
  new [path] <title>       Create new thread
  read <id>                Read thread content
  path <id>                Print thread file path
  stats [path]             Show thread statistics

  body <id>                Set/append body content (stdin)
  note <id> <sub> [args]   Manage notes (add/edit/remove)
  todo <id> <sub> [args]   Manage todos (add/check/uncheck/remove)
  log <id> <entry>         Add log entry

  status <id> <status>     Change thread status
  resolve <id>             Mark thread resolved
  reopen <id>              Reopen resolved thread
  update <id>              Update title/description
  remove <id>              Delete thread
  move <id> <path>         Move thread to different scope

  commit <id>              Commit specific thread
  commit --pending         Commit all modified threads
  validate [path]          Validate thread files

Options:
  -h, --help               Show this help
  -v, --version            Show version
  --commit                 Auto-commit after changes
  -m <msg>                 Commit message

Run 'threads <command> --help' for command-specific help.
HELP
    return 0;
}

# ============================================================================
# List command
# ============================================================================

sub cmd_list {
    my ($class, @args) = @_;

    my %opts = (
        recursive      => 0,
        down           => undef,  # undef = not set, integer = depth (0 = unlimited)
        up             => undef,  # undef = not set, integer = depth (0 = unlimited)
        search         => undef,
        status         => undef,
        include_closed => 0,
        json           => 0,
    );
    local @ARGV = @args;
    GetOptions(
        'r|recursive'     => \$opts{recursive},
        'd|down=i'        => \$opts{down},
        'u|up=i'          => \$opts{up},
        's|search=s'      => \$opts{search},
        'status=s'        => \$opts{status},
        'include-closed'  => \$opts{include_closed},
        'json'            => \$opts{json},
    ) or return 1;

    my $path = shift @ARGV // '.';
    my ($threads_dir, $cat, $proj, $level) = infer_scope($path);

    # Determine effective down/up depths:
    # - If --down is set, use it (0 = unlimited)
    # - If -r is set (without --down), treat as --down=0 (unlimited)
    # - Otherwise, no down search
    my $down_depth = undef;
    if (defined $opts{down}) {
        $down_depth = $opts{down};  # 0 = unlimited, N = N levels
    } elsif ($opts{recursive}) {
        $down_depth = 0;  # -r is alias for --down=0 (unlimited)
    }

    my $up_depth = $opts{up};  # undef = not set, 0 = unlimited, N = N levels

    my @files = find_all_threads(
        category         => $cat,
        project          => $proj,
        down_depth       => $down_depth,
        up_depth         => $up_depth,
        include_terminal => 1,  # Always load all threads
    );

    # Are we doing any direction search?
    my $is_searching = defined($down_depth) || defined($up_depth);

    # Load threads and filter (use lazy loading - only needs metadata)
    my @threads;
    for my $file (@files) {
        my $t = eval { Threads::Thread->new_from_file_lazy($file) };
        next unless $t;

        # Extract category/project from path
        my ($t_cat, $t_proj) = _extract_scope($file);

        # Filter by scope if not searching (local only)
        unless ($is_searching) {
            next if $cat eq '-' && $t_cat ne '-';
            next if $cat ne '-' && $proj eq '-' && $t_proj ne '-';
        }

        # Filter by status
        if ($opts{status}) {
            my @statuses = split /,/, $opts{status};
            my $base = $t->base_status;
            next unless grep { $_ eq $base } @statuses;
        } else {
            # If no status filter, exclude terminal statuses unless --include-closed
            next if !$opts{include_closed} && $t->is_terminal;
        }

        # Filter by search term
        if ($opts{search}) {
            my $term = lc $opts{search};
            my $match = (index(lc($t->name), $term) >= 0) ||
                        (index(lc($t->desc), $term) >= 0) ||
                        (index(lc($t->id), $term) >= 0);
            next unless $match;
        }

        # Compute relative path for this thread
        my $rel_path = _compute_relative_path($file);

        push @threads, {
            thread => $t,
            path   => $rel_path,
        };
    }

    if ($opts{json}) {
        require JSON::PP;
        my @data = map {{
            id     => $_->{thread}->id,
            name   => $_->{thread}->name,
            desc   => $_->{thread}->desc,
            status => $_->{thread}->status,
            path   => $_->{path},
        }} @threads;
        say JSON::PP::encode_json(\@data);
    } else {
        printf "Showing %d threads\n\n", scalar @threads;
        return 0 unless @threads;
        printf "%-8s %-10s %-24s %s\n", qw(ID STATUS PATH NAME);
        for my $item (@threads) {
            my $t = $item->{thread};
            my $path_display = _truncate($item->{path}, 22);
            printf "%-8s %-10s %-24s %s\n",
                $t->id,
                $t->base_status,
                $path_display,
                $t->name;
        }
    }

    return 0;
}
# ls - Alias for list
sub cmd_ls {
    my ($class, @args) = @_;
    return cmd_list($class, @args);
}

# ============================================================================
# Read command
# ============================================================================

sub cmd_read {
    my ($class, @args) = @_;

    my $id = shift @args or die "Usage: threads read <id>\n";
    my $path = find_thread($id);
    my $t = Threads::Thread->new_from_file($path);
    print $t->content;
    return 0;
}

# ============================================================================
# Path command
# ============================================================================

sub cmd_path {
    my ($class, @args) = @_;

    my %opts = (format => 'fancy');
    local @ARGV = @args;
    GetOptions(
        'f|format=s' => \$opts{format},
        'json'       => sub { $opts{format} = 'json' },
    ) or return 1;

    my $id = shift @ARGV or die "Usage: threads path <id>\n";
    my $ws = workspace_root();
    my $path = find_thread($id);
    my $abs_path = abs_path($path) // $path;
    my $rel_path = $path;
    $rel_path =~ s{^\Q$ws\E/}{};

    my $fmt = lc($opts{format});
    if ($fmt eq 'json') {
        require JSON::PP;
        say JSON::PP::encode_json({ path => $rel_path, path_absolute => $abs_path });
    } elsif ($fmt eq 'yaml') {
        require YAML::Tiny;
        my $yaml = YAML::Tiny->new({ path => $rel_path, path_absolute => $abs_path });
        print $yaml->write_string;
    } else {
        say $abs_path;
    }
    return 0;
}

# ============================================================================
# Stats command
# ============================================================================

sub cmd_stats {
    my ($class, @args) = @_;

    my %opts = (
        recursive => 0,
        down      => undef,
        up        => undef,
    );
    local @ARGV = @args;
    GetOptions(
        'r|recursive' => \$opts{recursive},
        'd|down=i'    => \$opts{down},
        'u|up=i'      => \$opts{up},
    ) or return 1;

    my $path = shift @ARGV // '.';
    my ($threads_dir, $cat, $proj, $level) = infer_scope($path);

    # Determine effective down/up depths (same logic as list)
    my $down_depth = undef;
    if (defined $opts{down}) {
        $down_depth = $opts{down};
    } elsif ($opts{recursive}) {
        $down_depth = 0;
    }

    my $up_depth = $opts{up};

    my @files = find_all_threads(
        category         => $cat,
        project          => $proj,
        down_depth       => $down_depth,
        up_depth         => $up_depth,
        include_terminal => 1,
    );

    my %counts;
    for my $file (@files) {
        my $t = eval { Threads::Thread->new_from_file_lazy($file) };
        next unless $t;
        $counts{$t->base_status}++;
    }

    say "| Status | Count |";
    say "|--------|-------|";
    for my $s (sort keys %counts) {
        printf "| %-6s | %5d |\n", $s, $counts{$s};
    }

    return 0;
}

# ============================================================================
# Validate command
# ============================================================================

sub cmd_validate {
    my ($class, @args) = @_;

    my %opts = (recursive => 0, format => 'fancy');
    local @ARGV = @args;
    GetOptions(
        'r|recursive' => \$opts{recursive},
        'f|format=s'  => \$opts{format},
        'json'        => sub { $opts{format} = 'json' },
    ) or return 1;

    my $path = shift @ARGV // '.';
    my $ws = workspace_root();
    my ($threads_dir, $cat, $proj, $level) = infer_scope($path);

    my @files = find_all_threads(
        category         => $cat,
        project          => $proj,
        recursive        => $opts{recursive},
        include_terminal => 1,
    );

    my @results;
    my $error_count = 0;

    for my $file (@files) {
        my $rel_path = $file;
        $rel_path =~ s{^\Q$ws\E/}{};
        my @issues;

        # validate uses full YAML parsing to catch parse errors
        my $t = eval { Threads::Thread->new_from_file($file) };
        unless ($t) {
            push @issues, "parse error";
        } else {
            push @issues, 'missing name/title field' unless $t->name;
            if (!$t->status) {
                push @issues, 'missing status field';
            } else {
                my $base = $t->base_status;
                unless (grep { $_ eq $base } @Threads::Thread::ALL_STATUSES) {
                    push @issues, "invalid status '$base'";
                }
            }
        }

        my $valid = @issues == 0;
        $error_count++ unless $valid;
        push @results, { path => $rel_path, valid => $valid, issues => \@issues };
    }

    my $fmt = lc($opts{format});
    if ($fmt eq 'json') {
        require JSON::PP;
        say JSON::PP::encode_json({ total => scalar(@results), errors => $error_count, results => \@results });
    } elsif ($fmt eq 'yaml') {
        require YAML::Tiny;
        # Convert to YAML-friendly structure
        my @yaml_results = map {
            { path => $_->{path}, valid => $_->{valid} ? 1 : 0, issues => $_->{issues} }
        } @results;
        my $yaml = YAML::Tiny->new({ total => scalar(@results), errors => $error_count, results => \@yaml_results });
        print $yaml->write_string;
    } else {
        for my $r (@results) {
            if ($r->{valid}) {
                say "OK: $r->{path}";
            } else {
                say "WARN: $r->{path}: " . join(', ', @{$r->{issues}});
            }
        }
        say $error_count ? "Validation failed with $error_count error(s)" : "All threads valid";
    }

    return $error_count ? 1 : 0;
}

# ============================================================================
# New command
# ============================================================================

sub cmd_new {
    my ($class, @args) = @_;

    my %opts = (status => 'idea', desc => '', body => undef, commit => 0, message => undef, format => 'fancy');
    local @ARGV = @args;
    GetOptions(
        'status=s'   => \$opts{status},
        'desc=s'     => \$opts{desc},
        'body=s'     => \$opts{body},
        'commit'     => \$opts{commit},
        'm=s'        => \$opts{message},
        'f|format=s' => \$opts{format},
        'json'       => sub { $opts{format} = 'json' },
    ) or return 1;

    my $fmt = lc($opts{format});

    # Parse positional: [path] title
    my ($path, $title);
    if (@ARGV >= 2) {
        $path = shift @ARGV;
        $title = shift @ARGV;
    } elsif (@ARGV == 1) {
        $path = '.';
        $title = shift @ARGV;
    } else {
        die "Usage: threads new [path] <title> [--desc=...] [--status=...]\n";
    }

    my $ws = workspace_root();
    my ($threads_dir, $cat, $proj, $level) = infer_scope($path);

    # Validate status
    unless (Threads::Thread::is_valid_status($opts{status})) {
        die "Invalid status '$opts{status}'. Must be one of: " . join(', ', @Threads::Thread::ALL_STATUSES) . "\n";
    }

    # Create thread
    my $thread = Threads::Thread->new(
        name   => $title,
        desc   => $opts{desc},
        status => $opts{status},
    );

    # Set body if provided
    if ($opts{body}) {
        $thread->set_body($opts{body});
    } elsif (!-t STDIN) {
        local $/;
        my $stdin = <STDIN>;
        $thread->set_body($stdin) if defined $stdin && length $stdin;
    }

    # Add initial log entry
    $thread->add_log_entry("Created thread.");

    # Save
    make_path($threads_dir) unless -d $threads_dir;
    my $filename = sprintf "%s-%s.md", $thread->id, slugify($title);
    my $filepath = "$threads_dir/$filename";
    $thread->save($filepath);

    my $rel_path = $filepath;
    $rel_path =~ s{^\Q$ws\E/}{};
    my $abs_path = abs_path($filepath) // $filepath;
    my $id = $thread->id;

    # Output based on format
    if ($fmt eq 'json') {
        require JSON::PP;
        say JSON::PP::encode_json({ id => $id, path => $rel_path, path_absolute => $abs_path });
    } elsif ($fmt eq 'yaml') {
        require YAML::Tiny;
        my $yaml = YAML::Tiny->new({ id => $id, path => $rel_path, path_absolute => $abs_path });
        print $yaml->write_string;
    } else {
        # Warn if no description (only in plain/fancy mode)
        warn "Warning: no description provided (use --desc)\n" unless length $opts{desc};

        say $id;
        _print_uncommitted_note($id, $opts{commit});
    }

    # Commit if requested
    if ($opts{commit}) {
        my $msg = $opts{message} // "threads: add $id - $title";
        git_commit([$filepath], $msg);
    }

    return 0;
}

# ============================================================================
# Body command
# ============================================================================

sub cmd_body {
    my ($class, @args) = @_;

    my %opts = (set => 1, append => 0, commit => 0, message => undef);
    local @ARGV = @args;
    GetOptions(
        'set'    => sub { $opts{set} = 1; $opts{append} = 0 },
        'append' => sub { $opts{append} = 1; $opts{set} = 0 },
        'commit' => \$opts{commit},
        'm=s'    => \$opts{message},
    ) or return 1;

    my $id = shift @ARGV or die "Usage: threads body <id> [--set|--append]\n";

    # Read content from stdin
    local $/;
    my $content = <STDIN>;
    die "No content provided (pipe content to stdin)\n"
        unless defined $content && length $content;

    my $path = find_thread($id);
    my $thread = Threads::Thread->new_from_file($path);

    if ($opts{append}) {
        $thread->append_body($content);
    } else {
        $thread->set_body($content);
    }

    $thread->save($path);

    if ($opts{commit}) {
        my $msg = $opts{message} // "threads: update " . $thread->id;
        git_commit([$path], $msg);
    }

    _print_uncommitted_note($thread->id, $opts{commit});
    return 0;
}

# ============================================================================
# Note command
# ============================================================================

sub cmd_note {
    my ($class, @args) = @_;

    my %opts = (commit => 0, message => undef);
    local @ARGV = @args;
    GetOptions(
        'commit' => \$opts{commit},
        'm=s'    => \$opts{message},
    ) or return 1;

    my $id = shift @ARGV or die "Usage: threads note <id> <add|edit|remove> ...\n";
    my $subcmd = shift @ARGV or die "Missing subcommand (add/edit/remove)\n";

    my $path = find_thread($id);
    my $thread = Threads::Thread->new_from_file($path);

    if ($subcmd eq 'add') {
        my $text = shift @ARGV or die "Missing note text\n";
        my $hash = $thread->add_note($text);
        $thread->add_log_entry("Added note.");
        say "Added to Notes: $text (id: $hash)";
    }
    elsif ($subcmd eq 'edit') {
        my $hash = shift @ARGV or die "Missing hash\n";
        my $text = shift @ARGV or die "Missing new text\n";
        $thread->edit_note($hash, $text);
        $thread->add_log_entry("Edited note $hash.");
        say "Edited note $hash";
    }
    elsif ($subcmd eq 'remove') {
        my $hash = shift @ARGV or die "Missing hash\n";
        $thread->remove_note($hash);
        $thread->add_log_entry("Removed note $hash.");
        say "Removed note $hash";
    }
    else {
        die "Unknown subcommand: $subcmd (expected add/edit/remove)\n";
    }

    $thread->save($path);

    if ($opts{commit}) {
        my $msg = $opts{message} // "threads: update " . $thread->id;
        git_commit([$path], $msg);
    }

    _print_uncommitted_note($thread->id, $opts{commit});
    return 0;
}

# ============================================================================
# Todo command
# ============================================================================

sub cmd_todo {
    my ($class, @args) = @_;

    my %opts = (commit => 0, message => undef);
    local @ARGV = @args;
    GetOptions(
        'commit' => \$opts{commit},
        'm=s'    => \$opts{message},
    ) or return 1;

    my $id = shift @ARGV or die "Usage: threads todo <id> <add|check|uncheck|remove> ...\n";
    my $subcmd = shift @ARGV or die "Missing subcommand (add/check/uncheck/remove)\n";

    my $path = find_thread($id);
    my $thread = Threads::Thread->new_from_file($path);

    if ($subcmd eq 'add') {
        my $text = shift @ARGV or die "Missing todo text\n";
        my $hash = $thread->add_todo($text);
        $thread->add_log_entry("Added todo.");
        say "Added to Todo: $text (id: $hash)";
    }
    elsif ($subcmd eq 'check') {
        my $hash = shift @ARGV or die "Missing hash\n";
        $thread->check_todo($hash);
        $thread->add_log_entry("Checked todo $hash.");
        say "Checked todo $hash";
    }
    elsif ($subcmd eq 'uncheck') {
        my $hash = shift @ARGV or die "Missing hash\n";
        $thread->uncheck_todo($hash);
        $thread->add_log_entry("Unchecked todo $hash.");
        say "Unchecked todo $hash";
    }
    elsif ($subcmd eq 'remove') {
        my $hash = shift @ARGV or die "Missing hash\n";
        $thread->remove_todo($hash);
        $thread->add_log_entry("Removed todo $hash.");
        say "Removed todo $hash";
    }
    else {
        die "Unknown subcommand: $subcmd (expected add/check/uncheck/remove)\n";
    }

    $thread->save($path);

    if ($opts{commit}) {
        my $msg = $opts{message} // "threads: update " . $thread->id;
        git_commit([$path], $msg);
    }

    _print_uncommitted_note($thread->id, $opts{commit});
    return 0;
}

# ============================================================================
# Log command
# ============================================================================

sub cmd_log {
    my ($class, @args) = @_;

    my %opts = (commit => 0, message => undef);
    local @ARGV = @args;
    GetOptions(
        'commit' => \$opts{commit},
        'm=s'    => \$opts{message},
    ) or return 1;

    my $id = shift @ARGV or die "Usage: threads log <id> <entry>\n";
    my $entry = shift @ARGV;

    # Read from stdin if no entry
    unless (defined $entry) {
        local $/;
        $entry = <STDIN>;
        chomp $entry if defined $entry;
    }
    die "No entry provided\n" unless defined $entry && length $entry;

    my $path = find_thread($id);
    my $thread = Threads::Thread->new_from_file($path);
    $thread->add_log_entry($entry);
    $thread->save($path);

    say "Logged to: $path";

    if ($opts{commit}) {
        my $msg = $opts{message} // "threads: update " . $thread->id;
        git_commit([$path], $msg);
    }

    _print_uncommitted_note($thread->id, $opts{commit});
    return 0;
}

# ============================================================================
# Status command
# ============================================================================

sub cmd_status {
    my ($class, @args) = @_;

    my %opts = (commit => 0, message => undef);
    local @ARGV = @args;
    GetOptions(
        'commit' => \$opts{commit},
        'm=s'    => \$opts{message},
    ) or return 1;

    my $id = shift @ARGV or die "Usage: threads status <id> <new-status>\n";
    my $new_status = shift @ARGV or die "Missing new status\n";

    # Validate status
    unless (Threads::Thread::is_valid_status($new_status)) {
        die "Invalid status '$new_status'. Must be one of: " . join(', ', @Threads::Thread::ALL_STATUSES) . "\n";
    }

    my $path = find_thread($id);
    my $thread = Threads::Thread->new_from_file($path);

    my $old_status = $thread->status;
    $thread->set_status($new_status);
    $thread->add_log_entry("Status: $old_status -> $new_status");
    $thread->save($path);

    say "Status changed: $old_status -> $new_status ($path)";

    if ($opts{commit}) {
        my $msg = $opts{message} // "threads: update " . $thread->id;
        git_commit([$path], $msg);
    }

    _print_uncommitted_note($thread->id, $opts{commit});
    return 0;
}

# ============================================================================
# Resolve command
# ============================================================================

sub cmd_resolve {
    my ($class, @args) = @_;

    my %opts = (commit => 0, message => undef);
    local @ARGV = @args;
    GetOptions(
        'commit' => \$opts{commit},
        'm=s'    => \$opts{message},
    ) or return 1;

    my $id = shift @ARGV or die "Usage: threads resolve <id>\n";

    my $path = find_thread($id);
    my $thread = Threads::Thread->new_from_file($path);

    $thread->set_status('resolved');
    $thread->add_log_entry("Resolved.");
    $thread->save($path);

    say "Resolved: " . $thread->name . " ($path)";

    if ($opts{commit}) {
        my $msg = $opts{message} // "threads: resolve " . $thread->id;
        git_commit([$path], $msg);
    }

    _print_uncommitted_note($thread->id, $opts{commit});
    return 0;
}

# ============================================================================
# Reopen command
# ============================================================================

sub cmd_reopen {
    my ($class, @args) = @_;

    my %opts = (status => 'active', commit => 0, message => undef);
    local @ARGV = @args;
    GetOptions(
        'status=s' => \$opts{status},
        'commit'   => \$opts{commit},
        'm=s'      => \$opts{message},
    ) or return 1;

    my $id = shift @ARGV or die "Usage: threads reopen <id> [--status=...]\n";

    # Validate status
    unless (Threads::Thread::is_valid_status($opts{status})) {
        die "Invalid status '$opts{status}'. Must be one of: " . join(', ', @Threads::Thread::ALL_STATUSES) . "\n";
    }

    my $path = find_thread($id);
    my $thread = Threads::Thread->new_from_file($path);

    $thread->set_status($opts{status});
    $thread->add_log_entry("Reopened as $opts{status}.");
    $thread->save($path);

    say "Reopened: " . $thread->name . " as $opts{status} ($path)";

    if ($opts{commit}) {
        my $msg = $opts{message} // "threads: reopen " . $thread->id;
        git_commit([$path], $msg);
    }

    _print_uncommitted_note($thread->id, $opts{commit});
    return 0;
}

# ============================================================================
# Update command
# ============================================================================

sub cmd_update {
    my ($class, @args) = @_;

    my %opts = (title => undef, desc => undef, commit => 0, message => undef);
    local @ARGV = @args;
    GetOptions(
        'title=s' => \$opts{title},
        'desc=s'  => \$opts{desc},
        'commit'  => \$opts{commit},
        'm=s'     => \$opts{message},
    ) or return 1;

    my $id = shift @ARGV or die "Usage: threads update <id> [--title=...] [--desc=...]\n";

    unless (defined $opts{title} || defined $opts{desc}) {
        die "Nothing to update (specify --title and/or --desc)\n";
    }

    my $path = find_thread($id);
    my $thread = Threads::Thread->new_from_file($path);

    my @changes;
    if (defined $opts{title}) {
        $thread->set_name($opts{title});
        push @changes, "title";
    }
    if (defined $opts{desc}) {
        $thread->set_desc($opts{desc});
        push @changes, "description";
    }

    $thread->add_log_entry("Updated " . join(', ', @changes) . ".");
    $thread->save($path);

    say "Updated: " . join(', ', @changes);

    if ($opts{commit}) {
        my $msg = $opts{message} // "threads: update " . $thread->id;
        git_commit([$path], $msg);
    }

    _print_uncommitted_note($thread->id, $opts{commit});
    return 0;
}

# ============================================================================
# Remove command
# ============================================================================

sub cmd_remove {
    my ($class, @args) = @_;

    my %opts = (force => 0, commit => 0, message => undef);
    local @ARGV = @args;
    GetOptions(
        'force'  => \$opts{force},
        'commit' => \$opts{commit},
        'm=s'    => \$opts{message},
    ) or return 1;

    my $id = shift @ARGV or die "Usage: threads remove <id> [--force]\n";

    my $path = find_thread($id);
    my $thread = Threads::Thread->new_from_file($path);

    unlink $path or die "Failed to remove: $!\n";
    say "Removed: $path";

    if ($opts{commit}) {
        my $rel = File::Spec->abs2rel($path, workspace_root());
        git_cmd('add', $rel);
        my $msg = $opts{message} // "threads: remove " . $thread->id;
        git_cmd('commit', '-m', $msg);
        git_cmd('pull', '--rebase');
        git_cmd('push');
    }

    return 0;
}

# ============================================================================
# Move command
# ============================================================================

sub cmd_move {
    my ($class, @args) = @_;

    my %opts = (commit => 0, message => undef);
    local @ARGV = @args;
    GetOptions(
        'commit' => \$opts{commit},
        'm=s'    => \$opts{message},
    ) or return 1;

    my $id = shift @ARGV or die "Usage: threads move <id> <path>\n";
    my $dest_path = shift @ARGV or die "Missing destination path\n";

    my $old_path = find_thread($id);
    my $thread = Threads::Thread->new_from_file($old_path);

    # Validate destination is within workspace
    my $ws = workspace_root();
    if (File::Spec->file_name_is_absolute($dest_path)) {
        my $ws_real = abs_path($ws) // $ws;
        my $dest_real = abs_path($dest_path);
        unless ($dest_real && $dest_real =~ /^\Q$ws_real\E/) {
            die "Invalid destination path: $dest_path\n";
        }
    }

    my ($threads_dir, $cat, $proj, $level) = infer_scope($dest_path);

    # Verify we can create the destination directory
    unless (-d $threads_dir) {
        eval { make_path($threads_dir) };
        if ($@ || !-d $threads_dir) {
            die "Cannot create destination: $threads_dir\n";
        }
    }

    my $filename = basename($old_path);
    my $new_path = "$threads_dir/$filename";

    rename $old_path, $new_path or die "Failed to move: $!\n";
    $thread->add_log_entry("Moved to $level.");
    $thread->{path} = $new_path;
    $thread->save($new_path);

    say "Moved: $old_path -> $new_path";

    if ($opts{commit}) {
        my $old_rel = File::Spec->abs2rel($old_path, workspace_root());
        my $new_rel = File::Spec->abs2rel($new_path, workspace_root());
        git_cmd('add', $old_rel, $new_rel);
        my $msg = $opts{message} // "threads: move " . $thread->id . " to $level";
        git_cmd('commit', '-m', $msg);
        git_cmd('pull', '--rebase');
        git_cmd('push');
    }

    _print_uncommitted_note($thread->id, $opts{commit});
    return 0;
}

# ============================================================================
# Git command
# ============================================================================

sub cmd_git {
    my ($class, @args) = @_;

    my @modified;
    my $ws = workspace_root();

    for my $file (find_all_threads(recursive => 1, include_terminal => 1)) {
        my $rel = File::Spec->abs2rel($file, $ws);

        # Check if file has uncommitted changes (using safe capture)
        my ($status) = git_capture('status', '--porcelain', '--', $rel);
        push @modified, $rel if $status;
    }

    if (@modified) {
        say "Pending thread changes:";
        say "  $_" for @modified;
    } else {
        say "No pending thread changes.";
    }

    return 0;
}

# ============================================================================
# Commit command
# ============================================================================

sub cmd_commit {
    my ($class, @args) = @_;

    my %opts = (pending => 0, message => undef);
    local @ARGV = @args;
    GetOptions(
        'pending' => \$opts{pending},
        'm=s'     => \$opts{message},
    ) or return 1;

    if ($opts{pending}) {
        git_commit_pending($opts{message});
        return 0;
    }

    my $id = shift @ARGV or die "Usage: threads commit <id> [-m msg] or threads commit --pending\n";

    my $path = find_thread($id);
    my $thread = Threads::Thread->new_from_file($path);
    my $msg = $opts{message} // "threads: update " . $thread->id;
    git_commit([$path], $msg);

    return 0;
}

# ============================================================================
# Helpers
# ============================================================================

sub _extract_scope {
    my ($filepath) = @_;
    my $ws = workspace_root();
    my $rel = File::Spec->abs2rel(dirname(dirname($filepath)), $ws);

    return ('-', '-') if $rel eq '.';

    my @parts = File::Spec->splitdir($rel);
    @parts = grep { $_ ne '' && $_ ne '.' } @parts;

    if (@parts == 0) {
        return ('-', '-');
    } elsif (@parts == 1) {
        return ($parts[0], '-');
    } else {
        return ($parts[0], $parts[1]);
    }
}

# Compute relative path from workspace root for a thread file
# Returns "." for workspace level, "cat" for category, "cat/proj" for project
sub _compute_relative_path {
    my ($filepath) = @_;
    my $ws = workspace_root();
    # Get parent of .threads directory (the scope directory)
    my $scope_dir = dirname(dirname($filepath));
    my $rel = File::Spec->abs2rel($scope_dir, $ws);

    return '.' if $rel eq '.' || $rel eq '';

    # Normalize path separators
    $rel =~ s{\\}{/}g;
    return $rel;
}

# Truncate string to max length with ellipsis
sub _truncate {
    my ($str, $max_len) = @_;
    return $str if length($str) <= $max_len;
    return substr($str, 0, $max_len - 2) . '..';
}

sub _print_uncommitted_note {
    my ($id, $committed) = @_;
    return if $committed;
    say "Note: Thread $id has uncommitted changes. Use 'threads commit $id' when ready.";
}

1;
