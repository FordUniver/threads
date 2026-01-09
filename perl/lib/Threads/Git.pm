package Threads::Git;
use strict;
use warnings;
use v5.16;

use Exporter 'import';
use File::Spec;
use Threads::Workspace qw(workspace_root);

our @EXPORT_OK = qw(
    git_commit
    git_commit_pending
    git_status
);

# Run git command in workspace
sub _workspace_git {
    my (@args) = @_;
    my $ws = workspace_root();
    system('git', '-C', $ws, @args);
    return $? >> 8;
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
        my $status = `git -C "$ws" status --porcelain -- "$rel" 2>/dev/null`;
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
    my $ws = workspace_root();
    my @status = `git -C "$ws" status --porcelain`;
    return grep { m{\.threads/} } @status;
}

1;
