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

# Get workspace root from WORKSPACE environment variable
sub workspace_root {
    my $ws = $ENV{WORKSPACE} // '';
    die "WORKSPACE environment variable not set\n" unless $ws;
    die "WORKSPACE directory does not exist: $ws\n" unless -d $ws;
    return $ws;
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
        $abs_path = abs_path($path) // $path;
    } else {
        $abs_path = abs_path(File::Spec->catdir(getcwd(), $path));
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
sub find_all_threads {
    my %opts = @_;
    my $recursive = $opts{recursive} // 0;
    my $scope_cat = $opts{category};
    my $scope_proj = $opts{project};
    my $include_terminal = $opts{include_terminal} // 0;

    my $ws = workspace_root();
    my @files;

    if ($recursive) {
        # Search all three levels
        push @files, _glob_thread_files($ws, '*.md');
        push @files, glob("$ws/*/.threads/*.md");
        push @files, glob("$ws/*/*/.threads/*.md");
    } elsif (defined $scope_cat && $scope_cat ne '-') {
        if (defined $scope_proj && $scope_proj ne '-') {
            # Project level only
            push @files, glob("$ws/$scope_cat/$scope_proj/.threads/*.md");
        } else {
            # Category level only
            push @files, glob("$ws/$scope_cat/.threads/*.md");
        }
    } else {
        # Workspace level only
        push @files, _glob_thread_files($ws, '*.md');
    }

    # Filter by terminal status unless include_terminal
    unless ($include_terminal) {
        @files = grep { !_is_terminal_status($_) } @files;
    }

    return @files;
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
        # Convert slug back to name (approximate)
        $slug =~ s/-/ /g;
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

    if ($content =~ /^status:\s*(resolved|superseded|deferred)/m) {
        return 1;
    }
    return 0;
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
