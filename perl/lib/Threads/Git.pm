package Threads::Git;
use strict;
use warnings;
use v5.16;

use Exporter 'import';
use File::Spec;
use IPC::Open3;
use Symbol 'gensym';
use Threads::Workspace qw(workspace_root);

our @EXPORT_OK = qw(
    git_commit
    git_commit_pending
    git_status
    git_cmd
    git_capture
);

# Run git command in workspace, die on failure
sub git_cmd {
    my (@args) = @_;
    my $ws = workspace_root();
    my $exit = system('git', '-C', $ws, @args);
    if ($exit != 0) {
        my $code = $? >> 8;
        die "git @args failed with exit code $code\n";
    }
    return 0;
}

# Run git command in workspace, return exit code without dying
sub _workspace_git {
    my (@args) = @_;
    my $ws = workspace_root();
    my $exit = system('git', '-C', $ws, @args);
    return $? >> 8;
}

# Capture git command output safely (no shell interpolation)
sub git_capture {
    my (@args) = @_;
    my $ws = workspace_root();

    my $err = gensym;
    my $pid = open3(my $in, my $out, $err, 'git', '-C', $ws, @args);
    close $in;

    my $output = do { local $/; <$out> };
    my $stderr = do { local $/; <$err> };

    waitpid($pid, 0);
    my $exit = $? >> 8;

    return wantarray ? ($output, $exit, $stderr) : $output;
}

# Commit specific files with message
sub git_commit {
    my ($files, $message) = @_;
    my $ws = workspace_root();

    # Stage files
    for my $file (@$files) {
        my $rel = File::Spec->abs2rel($file, $ws);
        _workspace_git('add', $rel);
    }

    # Commit
    my $exit = _workspace_git('commit', '-m', $message);
    return 1 if $exit != 0;

    # Pull and push
    _workspace_git('pull', '--rebase');
    _workspace_git('push');

    return 0;
}

# Commit all pending thread changes
sub git_commit_pending {
    my ($message) = @_;
    my $ws = workspace_root();

    # Find all thread files on disk and check for uncommitted changes
    my @thread_files;
    my @candidates = (
        glob("$ws/.threads/*.md"),
        glob("$ws/*/.threads/*.md"),
        glob("$ws/*/*/.threads/*.md"),
    );

    for my $file (@candidates) {
        my $rel = File::Spec->abs2rel($file, $ws);
        my ($status) = git_capture('status', '--porcelain', '--', $rel);
        push @thread_files, $rel if $status;
    }

    return 0 unless @thread_files;

    # Stage thread files
    for my $file (@thread_files) {
        _workspace_git('add', $file);
    }

    # Commit
    $message //= 'threads: update pending';
    my $exit = _workspace_git('commit', '-m', $message);
    return 1 if $exit != 0;

    # Pull and push
    _workspace_git('pull', '--rebase');
    _workspace_git('push');

    return 0;
}

# Get git status for thread files
sub git_status {
    my ($output) = git_capture('status', '--porcelain');
    my @status = split /\n/, ($output // '');
    return grep { m{\.threads/} } @status;
}

1;
