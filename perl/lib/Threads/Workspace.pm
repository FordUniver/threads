package Threads::Workspace;
use strict;
use warnings;
use v5.16;

use Exporter 'import';
use Cwd qw(abs_path getcwd);
use File::Spec;
use File::Basename qw(basename);

our @EXPORT_OK = qw(
    workspace_root
    infer_scope
    find_thread
    find_all_threads
    slugify
);

# Precompiled pattern for terminal status detection (quick check)
our $TERMINAL_STATUS_RE = qr/^status:\s*(resolved|superseded|deferred|rejected)/m;

# Cache for workspace root (process-scoped, computed once per invocation)
my $_workspace_root_cache;

# Get workspace root via git rev-parse (like other implementations)
# Result is cached to avoid repeated subprocess spawns
sub workspace_root {
    return $_workspace_root_cache if defined $_workspace_root_cache;

    my $output = `git rev-parse --show-toplevel 2>/dev/null`;
    if ($? != 0 || !defined $output || $output eq '') {
        die "Not in a git repository. threads requires a git repo to define scope.\n";
    }
    chomp $output;
    die "Git root directory does not exist: $output\n" unless -d $output;

    $_workspace_root_cache = $output;
    return $output;
}

# Infer scope from path, returns (threads_dir, category, project, level_desc)
sub infer_scope {
    my ($path) = @_;
    $path //= '.';

    my $ws = workspace_root();
    # Resolve symlinks for workspace
    my $ws_real = abs_path($ws) // $ws;

    # Resolve to absolute path (with symlinks resolved)
    my $abs_path;
    if ($path eq '.') {
        $abs_path = abs_path(getcwd()) // getcwd();
    } elsif (File::Spec->file_name_is_absolute($path)) {
        $abs_path = abs_path($path);
        die "Path not found: $path\n" unless defined $abs_path && -d $abs_path;
    } else {
        my $full_path = File::Spec->catdir(getcwd(), $path);
        $abs_path = abs_path($full_path);
        # Try git-root-relative path if PWD-relative doesn't work
        if (!defined $abs_path || !-d $abs_path) {
            my $ws_path = File::Spec->catdir($ws, $path);
            $abs_path = abs_path($ws_path);
            die "Path not found: $path\n" unless defined $abs_path && -d $abs_path;
        }
    }

    # If path is exactly workspace, return workspace level
    if ($abs_path eq $ws_real) {
        return ("$ws/.threads", '-', '-', 'workspace');
    }

    # Verify path is within workspace
    my $rel = File::Spec->abs2rel($abs_path, $ws_real);
    if ($rel =~ /^\.\./) {
        # Default to workspace level when path is outside (matches shell behavior)
        warn "Warning: cwd outside workspace, defaulting to workspace level\n";
        return ("$ws/.threads", '-', '-', 'workspace');
    }

    # Split relative path into components (max 2 levels: category/project)
    my @parts = File::Spec->splitdir($rel);
    @parts = grep { $_ ne '' && $_ ne '.' } @parts;

    my ($category, $project, $level);

    if (@parts == 0) {
        # Workspace level
        return ("$ws/.threads", '-', '-', 'workspace');
    } elsif (@parts == 1) {
        # Category level
        $category = $parts[0];
        $project = '-';
        $level = "category ($category)";
        return ("$ws/$category/.threads", $category, $project, $level);
    } else {
        # Project level (ignore deeper paths)
        $category = $parts[0];
        $project = $parts[1];
        $level = "project ($category/$project)";
        return ("$ws/$category/$project/.threads", $category, $project, $level);
    }
}

# Find a thread by ID or name
sub find_thread {
    my ($id_or_name) = @_;
    die "Thread identifier required\n" unless defined $id_or_name && length $id_or_name;

    my $ws = workspace_root();

    # Try ID-based lookup first (6-char hex prefix)
    if ($id_or_name =~ /^[0-9a-f]{6}$/i) {
        my @matches = _glob_all_levels($ws, "$id_or_name-*.md");
        return $matches[0] if @matches == 1;
        if (@matches > 1) {
            die "Ambiguous thread ID '$id_or_name': multiple matches\n";
        }
    }

    # Fall back to name search
    my @all = find_all_threads(recursive => 1, include_terminal => 1);
    my @name_matches;

    for my $path (@all) {
        my ($id, $name) = _thread_id_name($path);
        if (lc($name) eq lc($id_or_name) || index(lc($name), lc($id_or_name)) >= 0) {
            push @name_matches, [$path, $id, $name];
        }
    }

    return $name_matches[0][0] if @name_matches == 1;

    if (@name_matches > 1) {
        die "Ambiguous thread name '$id_or_name'. Candidates:\n" .
            join('', map { "  $_->[1]  $_->[2]\n" } @name_matches);
    }

    die "Thread not found: $id_or_name\n";
}

# Find all threads with optional filtering
# Supports Phase 3/4 direction flags:
#   down_depth: undef = no down search, 0 = unlimited, N = N levels
#   up_depth:   undef = no up search, 0 = unlimited (to git root), N = N levels
sub find_all_threads {
    my %opts = @_;
    my $scope_cat = $opts{category};
    my $scope_proj = $opts{project};
    my $include_terminal = $opts{include_terminal} // 0;
    my $down_depth = $opts{down_depth};  # undef, 0 (unlimited), or N
    my $up_depth = $opts{up_depth};      # undef, 0 (unlimited), or N

    # Legacy support: 'recursive' maps to down_depth=0
    if ($opts{recursive} && !defined $down_depth) {
        $down_depth = 0;
    }

    my $ws = workspace_root();
    my %seen;
    my @files;

    # Determine start path based on scope
    my $start_path;
    if (defined $scope_cat && $scope_cat ne '-') {
        if (defined $scope_proj && $scope_proj ne '-') {
            $start_path = "$ws/$scope_cat/$scope_proj";
        } else {
            $start_path = "$ws/$scope_cat";
        }
    } else {
        $start_path = $ws;
    }

    # Always collect threads at start path
    _collect_threads_at_path($start_path, \@files, \%seen);

    # Search down (subdirectories) if requested
    if (defined $down_depth) {
        _find_threads_down($start_path, $ws, \@files, \%seen, 0, $down_depth);
    }

    # Search up (parent directories) if requested
    if (defined $up_depth) {
        _find_threads_up($start_path, $ws, \@files, \%seen, 0, $up_depth);
    }

    # Filter by terminal status unless include_terminal
    unless ($include_terminal) {
        @files = grep { !_is_terminal_status($_) } @files;
    }

    return sort @files;
}

# Collect thread files from a .threads directory at the given path
sub _collect_threads_at_path {
    my ($dir, $files, $seen) = @_;
    my $threads_dir = "$dir/.threads";
    return unless -d $threads_dir;

    my @found = glob("$threads_dir/*.md");
    for my $f (@found) {
        next if $seen->{$f}++;
        push @$files, $f;
    }
}

# Recursively find threads going DOWN into subdirectories
sub _find_threads_down {
    my ($dir, $ws, $files, $seen, $current_depth, $max_depth) = @_;

    # Check depth limit: max_depth=0 means unlimited, N means stop at N
    if ($max_depth > 0 && $current_depth >= $max_depth) {
        return;
    }

    opendir(my $dh, $dir) or return;
    my @entries = readdir($dh);
    closedir($dh);

    for my $entry (@entries) {
        next if $entry =~ /^\./;  # Skip hidden dirs
        my $subdir = "$dir/$entry";
        next unless -d $subdir;

        # Stop at nested git repos (git boundary) - check for .git dir or file (worktree)
        if (-e "$subdir/.git") {
            next;
        }

        # Collect threads at this level
        _collect_threads_at_path($subdir, $files, $seen);

        # Continue recursing
        _find_threads_down($subdir, $ws, $files, $seen, $current_depth + 1, $max_depth);
    }
}

# Find threads going UP into parent directories
sub _find_threads_up {
    my ($dir, $ws, $files, $seen, $current_depth, $max_depth) = @_;

    # Check depth limit: max_depth=0 means unlimited (to git root), N means N levels
    if ($max_depth > 0 && $current_depth >= $max_depth) {
        return;
    }

    # Get parent directory
    my $parent = File::Spec->catdir($dir, '..');
    $parent = abs_path($parent);
    return unless defined $parent && -d $parent;

    # Stop at workspace root boundary (don't go above it)
    my $ws_real = abs_path($ws) // $ws;
    my $parent_real = abs_path($parent) // $parent;

    # Check if parent is still within or at the workspace
    my $rel = File::Spec->abs2rel($parent_real, $ws_real);
    if ($rel =~ /^\.\./) {
        # Parent is outside workspace, stop
        return;
    }

    # Collect threads at parent
    _collect_threads_at_path($parent, $files, $seen);

    # Continue up
    _find_threads_up($parent, $ws, $files, $seen, $current_depth + 1, $max_depth);
}

# Glob for thread files at workspace level
sub _glob_thread_files {
    my ($ws, $pattern) = @_;
    return glob("$ws/.threads/$pattern");
}

# Glob for thread files at all levels (workspace, category, project)
sub _glob_all_levels {
    my ($ws, $pattern) = @_;
    my @files;
    push @files, glob("$ws/.threads/$pattern");
    push @files, glob("$ws/*/.threads/$pattern");
    push @files, glob("$ws/*/*/.threads/$pattern");
    return @files;
}

# Extract thread ID and name from filename
sub _thread_id_name {
    my ($path) = @_;
    my $filename = basename($path, '.md');
    if ($filename =~ /^([0-9a-f]{6})-(.+)$/) {
        my ($id, $slug) = ($1, $2);
        return ($id, $slug);
    }
    return (undef, $filename);
}

# Check if thread has terminal status (quick check without full parse)
sub _is_terminal_status {
    my ($path) = @_;
    open my $fh, '<', $path or return 0;
    my $content = '';
    # Read first 500 bytes (enough for frontmatter)
    read $fh, $content, 500;
    close $fh;

    return $content =~ $TERMINAL_STATUS_RE ? 1 : 0;
}

# Convert title to filename slug
sub slugify {
    my ($text) = @_;
    $text = lc $text;
    $text =~ s/[^a-z0-9-]+/-/g;
    $text =~ s/-+/-/g;
    $text =~ s/^-|-$//g;
    return $text;
}

1;
