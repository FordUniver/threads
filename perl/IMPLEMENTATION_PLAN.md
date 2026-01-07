# Perl Implementation Plan

## Overview

Rewrite the threads CLI in Perl for the language comparison study. Target: ~600-800 lines with YAML::Tiny as the only CPAN dependency.

## Architecture

```
perl/
├── bin/threads                 # Entry point (~50 lines)
├── lib/Threads/
│   ├── CLI.pm                  # Command dispatch (~200 lines)
│   ├── Thread.pm               # Thread object (~150 lines)
│   ├── Workspace.pm            # Path resolution, discovery (~120 lines)
│   ├── Section.pm              # Markdown section ops (~80 lines)
│   └── Git.pm                  # Git operations (~60 lines)
├── cpanfile                    # YAML::Tiny dependency
├── t/                          # Tests
│   ├── 00-load.t
│   ├── thread.t
│   ├── workspace.t
│   ├── section.t
│   └── cli.t
└── Makefile                    # Install, test, lint targets
```

Estimated total: ~660 lines (excluding tests).

---

## Phase 1: Core Infrastructure

### 1.1 Entry Point (`bin/threads`)

```perl
#!/usr/bin/env perl
use strict;
use warnings;
use v5.16;

use FindBin;
use lib "$FindBin::Bin/../lib";
use Threads::CLI;

exit Threads::CLI->run(@ARGV);
```

Key points:
- `FindBin` locates script directory for portable `lib` path
- Exit code from CLI module propagates to shell
- No argument processing here—delegate entirely to CLI

### 1.2 Workspace Module (`lib/Threads/Workspace.pm`)

Core responsibility: path resolution and thread discovery.

**Key functions:**

```perl
sub workspace_root {
    # Return $ENV{WORKSPACE} or die
}

sub infer_scope {
    my ($path) = @_;
    # Returns: ($threads_dir, $category, $project, $level_desc)
    # Logic:
    # 1. "." → workspace level (cat="-", proj="-")
    # 2. Resolve to absolute, verify within $WORKSPACE
    # 3. Extract 0-2 path components relative to $WORKSPACE
    # 4. Build threads_dir path
}

sub find_thread {
    my ($id_or_name) = @_;
    # Returns: $filepath or dies
    # 1. Glob for ID-prefixed files: $WORKSPACE/**/.threads/${id}*.md
    # 2. If unique match, return it
    # 3. If ambiguous, search all threads by name
    # 4. Error with candidates if still ambiguous
}

sub find_all_threads {
    my (%opts) = @_;  # scope, recursive
    # Returns: @filepaths
    # Use glob patterns (faster than File::Find for known structure)
}
```

**Critical detail:** The bash version uses three glob patterns:
```
$WORKSPACE/.threads/*.md           # workspace level
$WORKSPACE/*/.threads/*.md         # category level
$WORKSPACE/*/*/.threads/*.md       # project level
```

Perl equivalent with `glob()`:
```perl
my @patterns = (
    "$ws/.threads/*.md",
    "$ws/*/.threads/*.md",
    "$ws/*/*/.threads/*.md",
);
my @files = map { glob($_) } @patterns;
```

### 1.3 Thread Module (`lib/Threads/Thread.pm`)

Object representing a single thread file.

**Constructor patterns:**

```perl
sub new_from_file {
    my ($class, $path) = @_;
    my $content = read_file($path);
    my ($meta, $body) = parse_frontmatter($content);
    return bless {
        path    => $path,
        id      => $meta->{id},
        name    => $meta->{name},
        desc    => $meta->{desc}  // '',
        status  => $meta->{status},
        body    => $body,
        _dirty  => 0,
    }, $class;
}

sub new {
    my ($class, %args) = @_;
    my $id = $args{id} // generate_id();
    return bless {
        id      => $id,
        name    => $args{name},
        desc    => $args{desc} // '',
        status  => $args{status} // 'idea',
        body    => initial_body(),
        _dirty  => 1,
    }, $class;
}
```

**Frontmatter parsing (native regex):**

```perl
sub parse_frontmatter {
    my ($content) = @_;
    return (undef, $content) unless $content =~ /\A---\n(.+?)\n---\n(.*)/s;
    my ($yaml_str, $body) = ($1, $2);
    my $meta = YAML::Tiny->read_string("---\n$yaml_str\n")->[0];
    return ($meta, $body);
}
```

**Frontmatter serialization:**

```perl
sub to_frontmatter {
    my ($self) = @_;
    my $yaml = YAML::Tiny->new({
        id     => $self->{id},
        name   => $self->{name},
        desc   => $self->{desc},
        status => $self->{status},
    });
    my $str = $yaml->write_string;
    $str =~ s/^---\n//;  # YAML::Tiny adds ---
    return "---\n$str---\n";
}
```

**ID generation:**

```perl
sub generate_id {
    # Match bash: 6 hex chars from /dev/urandom
    open my $fh, '<', '/dev/urandom' or die;
    read $fh, my $bytes, 3;
    close $fh;
    return unpack('H6', $bytes);
}
```

**Hash generation (for notes/todos):**

```perl
sub generate_hash {
    my ($text) = @_;
    require Digest::MD5;
    my $input = $text . time() . $$;  # text + timestamp + pid
    return substr(Digest::MD5::md5_hex($input), 0, 4);
}
```

### 1.4 Section Module (`lib/Threads/Section.pm`)

Markdown section manipulation—Perl's sweet spot.

```perl
sub get_section {
    my ($content, $section) = @_;
    if ($content =~ /^##\s*$section\s*\n(.*?)(?=^##\s|\z)/ms) {
        return $1;
    }
    return '';
}

sub set_section {
    my ($content, $section, $new_text) = @_;
    $new_text =~ s/\n*$/\n/;  # Ensure trailing newline

    if ($content =~ /^##\s*$section\s*\n/m) {
        $content =~ s{
            (^##\s*$section\s*\n)  # Section header
            .*?                    # Existing content
            (?=^##\s|\z)           # Until next section or EOF
        }{$1$new_text}xms;
    } else {
        # Append new section
        $content =~ s/\n*$/\n\n## $section\n$new_text/;
    }
    return $content;
}

sub append_to_section {
    my ($content, $section, $text) = @_;
    my $existing = get_section($content, $section);
    return set_section($content, $section, $existing . $text);
}
```

---

## Phase 2: Read-Only Commands

### 2.1 `list` Command

```perl
sub cmd_list {
    my ($self, @args) = @_;

    my %opts = (
        recursive => 0,
        search    => undef,
        status    => undef,
        all       => 0,
        json      => 0,
    );

    GetOptionsFromArray(\@args,
        'r|recursive' => \$opts{recursive},
        's|search=s'  => \$opts{search},
        'status=s'    => \$opts{status},
        'all'         => \$opts{all},
        'json'        => \$opts{json},
    ) or return 1;

    my $path = shift @args // '.';
    my ($threads_dir, $cat, $proj, $level) = infer_scope($path);

    my @threads = find_all_threads(
        scope     => [$cat, $proj],
        recursive => $opts{recursive},
    );

    # Filter by status (exclude terminal unless --all)
    @threads = grep { ... } @threads unless $opts{all};

    # Filter by search term
    @threads = grep { ... } @threads if $opts{search};

    # Output
    if ($opts{json}) {
        say encode_json(\@threads);
    } else {
        print_table(\@threads);
    }

    return 0;
}
```

**Table formatting:** Use `printf` with fixed column widths:
```perl
printf "%-8s %-10s %-12s %-12s %s\n", qw(ID STATUS CATEGORY PROJECT NAME);
```

### 2.2 `read` Command

Simplest command—just output file contents:

```perl
sub cmd_read {
    my ($self, @args) = @_;
    my $id = shift @args or die "Usage: threads read <id>\n";
    my $path = find_thread($id);
    print read_file($path);
    return 0;
}
```

### 2.3 `stats` Command

```perl
sub cmd_stats {
    my ($self, @args) = @_;

    my $recursive = 0;
    GetOptionsFromArray(\@args, 'r|recursive' => \$recursive);

    my @threads = find_all_threads(recursive => $recursive);

    my %counts;
    for my $t (@threads) {
        my $status = $t->status =~ s/\s.*//r;  # Strip reason
        $counts{$status}++;
    }

    say "| Status | Count |";
    say "|--------|-------|";
    for my $s (sort keys %counts) {
        printf "| %-6s | %5d |\n", $s, $counts{$s};
    }

    return 0;
}
```

### 2.4 `validate` Command

```perl
sub cmd_validate {
    my ($self, @args) = @_;

    my @threads = find_all_threads(...);
    my $errors = 0;

    my @required = qw(name status);
    my @valid_statuses = qw(idea planning active blocked paused resolved superseded deferred);

    for my $t (@threads) {
        for my $field (@required) {
            unless ($t->$field) {
                warn "$t->{path}: missing $field\n";
                $errors++;
            }
        }
        my $base_status = $t->status =~ s/\s.*//r;
        unless (grep { $_ eq $base_status } @valid_statuses) {
            warn "$t->{path}: invalid status '$base_status'\n";
            $errors++;
        }
    }

    return $errors ? 1 : 0;
}
```

---

## Phase 3: Write Commands

### 3.1 `new` Command

```perl
sub cmd_new {
    my ($self, @args) = @_;

    my %opts = (status => 'idea', desc => '', body => undef, commit => 0, message => undef);
    GetOptionsFromArray(\@args,
        'status=s' => \$opts{status},
        'desc=s'   => \$opts{desc},
        'body=s'   => \$opts{body},
        'commit'   => \$opts{commit},
        'm=s'      => \$opts{message},
    );

    # Parse positional args: [path] title
    my ($path, $title);
    if (@args == 2) {
        ($path, $title) = @args;
    } elsif (@args == 1) {
        $path = '.';
        $title = shift @args;
    } else {
        die "Usage: threads new [path] <title>\n";
    }

    my ($threads_dir, $cat, $proj, $level) = infer_scope($path);

    # Create thread
    my $thread = Threads::Thread->new(
        name   => $title,
        desc   => $opts{desc},
        status => $opts{status},
    );

    # Add initial body if provided
    $thread->set_body($opts{body}) if $opts{body};

    # Read body from stdin if available
    unless ($opts{body} || -t STDIN) {
        local $/;
        my $stdin = <STDIN>;
        $thread->set_body($stdin) if $stdin;
    }

    # Add initial log entry
    $thread->add_log_entry("Created thread.");

    # Save
    my $filename = sprintf "%s-%s.md", $thread->id, slugify($title);
    my $filepath = "$threads_dir/$filename";
    mkdir $threads_dir unless -d $threads_dir;
    $thread->save($filepath);

    # Warn if no description
    warn "Warning: no description provided (use --desc)\n" unless $opts{desc};

    # Commit if requested
    if ($opts{commit}) {
        git_commit([$filepath], $opts{message} // "threads: add $thread->{id} - $title");
    }

    say $thread->id;
    return 0;
}
```

**Slugify function:**

```perl
sub slugify {
    my ($text) = @_;
    $text = lc $text;
    $text =~ s/[^a-z0-9-]+/-/g;
    $text =~ s/-+/-/g;
    $text =~ s/^-|-$//g;
    return $text;
}
```

### 3.2 `body` Command

```perl
sub cmd_body {
    my ($self, @args) = @_;

    my %opts = (set => 1, append => 0, commit => 0, message => undef);
    GetOptionsFromArray(\@args,
        'set'    => sub { $opts{set} = 1; $opts{append} = 0 },
        'append' => sub { $opts{append} = 1; $opts{set} = 0 },
        'commit' => \$opts{commit},
        'm=s'    => \$opts{message},
    );

    my $id = shift @args or die "Usage: threads body <id> [--set|--append]\n";

    # Read content from stdin
    local $/;
    my $content = <STDIN>;
    die "No content provided\n" unless defined $content && length $content;

    my $path = find_thread($id);
    my $thread = Threads::Thread->new_from_file($path);

    if ($opts{append}) {
        $thread->append_body($content);
    } else {
        $thread->set_body($content);
    }

    $thread->save($path);

    if ($opts{commit}) {
        git_commit([$path], $opts{message});
    }

    return 0;
}
```

### 3.3 `note` Command

```perl
sub cmd_note {
    my ($self, @args) = @_;

    my %opts = (commit => 0, message => undef);
    GetOptionsFromArray(\@args,
        'commit' => \$opts{commit},
        'm=s'    => \$opts{message},
    );

    my $id = shift @args or die "Usage: threads note <id> <add|edit|remove> ...\n";
    my $subcmd = shift @args or die "Missing subcommand\n";

    my $path = find_thread($id);
    my $thread = Threads::Thread->new_from_file($path);

    if ($subcmd eq 'add') {
        my $text = shift @args or die "Missing note text\n";
        $thread->add_note($text);
        $thread->add_log_entry("Added note.");
    }
    elsif ($subcmd eq 'edit') {
        my $hash = shift @args or die "Missing hash\n";
        my $text = shift @args or die "Missing new text\n";
        $thread->edit_note($hash, $text);
        $thread->add_log_entry("Edited note $hash.");
    }
    elsif ($subcmd eq 'remove') {
        my $hash = shift @args or die "Missing hash\n";
        $thread->remove_note($hash);
        $thread->add_log_entry("Removed note $hash.");
    }
    else {
        die "Unknown subcommand: $subcmd\n";
    }

    $thread->save($path);

    if ($opts{commit}) {
        git_commit([$path], $opts{message});
    }

    return 0;
}
```

**Note manipulation in Thread.pm:**

```perl
sub add_note {
    my ($self, $text) = @_;
    my $hash = generate_hash($text);
    my $line = "- $text  <!-- $hash -->\n";
    $self->{body} = append_to_section($self->{body}, 'Notes', $line);
}

sub edit_note {
    my ($self, $hash, $new_text) = @_;
    $self->{body} =~ s/^(- ).*?(  <!-- $hash -->)$/$1$new_text$2/m
        or die "Note $hash not found\n";
}

sub remove_note {
    my ($self, $hash) = @_;
    $self->{body} =~ s/^- .*?  <!-- $hash -->\n//m
        or die "Note $hash not found\n";
}
```

### 3.4 `todo` Command

Same pattern as `note`, with checkbox handling:

```perl
sub add_todo {
    my ($self, $text) = @_;
    my $hash = generate_hash($text);
    my $line = "- [ ] $text  <!-- $hash -->\n";
    $self->{body} = append_to_section($self->{body}, 'Todo', $line);
}

sub check_todo {
    my ($self, $hash) = @_;
    $self->{body} =~ s/^(- )\[ \](.*)<!-- $hash -->/$1\[x\]$2<!-- $hash -->/m
        or die "Todo $hash not found or already checked\n";
}

sub uncheck_todo {
    my ($self, $hash) = @_;
    $self->{body} =~ s/^(- )\[x\](.*)<!-- $hash -->/$1\[ \]$2<!-- $hash -->/m
        or die "Todo $hash not found or already unchecked\n";
}
```

### 3.5 `log` Command

```perl
sub cmd_log {
    my ($self, @args) = @_;

    my %opts = (commit => 0, message => undef);
    GetOptionsFromArray(\@args,
        'commit' => \$opts{commit},
        'm=s'    => \$opts{message},
    );

    my $id = shift @args or die "Usage: threads log <id> <entry>\n";
    my $entry = shift @args;

    # Read from stdin if no entry provided
    unless ($entry) {
        local $/;
        $entry = <STDIN>;
        chomp $entry if $entry;
    }
    die "No entry provided\n" unless $entry;

    my $path = find_thread($id);
    my $thread = Threads::Thread->new_from_file($path);
    $thread->add_log_entry($entry);
    $thread->save($path);

    if ($opts{commit}) {
        git_commit([$path], $opts{message});
    }

    return 0;
}
```

**Log entry formatting in Thread.pm:**

```perl
sub add_log_entry {
    my ($self, $entry) = @_;

    my ($sec, $min, $hour, $mday, $mon, $year) = localtime;
    my $date = sprintf "%04d-%02d-%02d", $year + 1900, $mon + 1, $mday;
    my $time = sprintf "%02d:%02d", $hour, $min;

    my $log = get_section($self->{body}, 'Log');

    # Check if today's date header exists
    if ($log =~ /^### $date$/m) {
        # Append under existing date
        $self->{body} =~ s/(### $date\n.*?)(?=\n### |\n## |\z)/$1- **$time** $entry\n/s;
    } else {
        # Add new date header
        my $new_entry = "### $date\n\n- **$time** $entry\n";
        $self->{body} = append_to_section($self->{body}, 'Log', "\n$new_entry");
    }
}
```

### 3.6 Remaining Write Commands

**`status`**: Update status field, add log entry.

**`resolve`**: Set status to "resolved", add "Resolved." log entry.

**`reopen`**: Set status (default: "active"), add "Reopened." log entry.

**`update`**: Modify title and/or desc fields.

**`remove`**: Delete thread file (with confirmation unless --force).

**`move`**: Move thread to different scope (rename file path).

---

## Phase 4: Git Integration

### 4.1 Git Module (`lib/Threads/Git.pm`)

```perl
package Threads::Git;
use strict;
use warnings;
use v5.16;

sub workspace_git {
    my (@args) = @_;
    my $ws = $ENV{WORKSPACE} or die "WORKSPACE not set\n";
    system('git', '-C', $ws, @args);
    return $? >> 8;
}

sub commit {
    my ($files, $message) = @_;
    my $ws = $ENV{WORKSPACE};

    # Stage files
    for my $file (@$files) {
        my $rel = File::Spec->abs2rel($file, $ws);
        workspace_git('add', $rel);
    }

    # Commit
    workspace_git('commit', '-m', $message);

    # Pull and push
    workspace_git('pull', '--rebase');
    workspace_git('push');
}

sub commit_pending {
    my ($message) = @_;
    my $ws = $ENV{WORKSPACE};

    # Find modified .threads files
    my @status = `git -C "$ws" status --porcelain`;
    my @thread_files = grep { m{\.threads/.*\.md$} } @status;

    return 0 unless @thread_files;

    # Stage and commit
    workspace_git('add', '-A', '**/.threads/*.md');
    workspace_git('commit', '-m', $message // 'threads: update pending');
    workspace_git('pull', '--rebase');
    workspace_git('push');
}
```

### 4.2 `commit` Command

```perl
sub cmd_commit {
    my ($self, @args) = @_;

    my %opts = (message => undef, pending => 0);
    GetOptionsFromArray(\@args,
        'm=s'     => \$opts{message},
        'pending' => \$opts{pending},
    );

    if ($opts{pending}) {
        return git_commit_pending($opts{message});
    }

    my $id = shift @args or die "Usage: threads commit <id> [-m msg]\n";
    my $path = find_thread($id);
    my $thread = Threads::Thread->new_from_file($path);

    my $msg = $opts{message} // "threads: update $thread->{id}";
    git_commit([$path], $msg);

    return 0;
}
```

---

## Phase 5: Testing

### 5.1 Test Structure

Mirror the bash test structure:

```
t/
├── 00-load.t           # Module loading
├── thread.t            # Thread parsing/serialization
├── workspace.t         # Path resolution
├── section.t           # Section manipulation
├── cli/
│   ├── list.t
│   ├── new.t
│   ├── read.t
│   ├── body.t
│   ├── note.t
│   ├── todo.t
│   ├── log.t
│   └── lifecycle.t     # status, resolve, reopen
└── integration.t       # End-to-end scenarios
```

### 5.2 Test Helpers

```perl
# t/lib/TestHelper.pm
package TestHelper;
use strict;
use warnings;
use File::Temp qw(tempdir);
use File::Path qw(make_path);

sub setup_workspace {
    my $dir = tempdir(CLEANUP => 1);
    $ENV{WORKSPACE} = $dir;
    make_path("$dir/.threads");
    return $dir;
}

sub create_thread {
    my ($ws, %args) = @_;
    # Create thread file with given attributes
}

1;
```

### 5.3 Example Test

```perl
# t/thread.t
use strict;
use warnings;
use Test::More;
use lib 't/lib';
use TestHelper;
use Threads::Thread;

my $ws = TestHelper::setup_workspace();

subtest 'parse frontmatter' => sub {
    my $content = <<'END';
---
id: abc123
name: Test Thread
desc: A test
status: active
---

## Body

Content here.
END

    my $thread = Threads::Thread->new_from_string($content);
    is $thread->id, 'abc123';
    is $thread->name, 'Test Thread';
    is $thread->status, 'active';
};

subtest 'generate ID' => sub {
    my $id = Threads::Thread::generate_id();
    like $id, qr/^[0-9a-f]{6}$/;
};

done_testing;
```

---

## Phase 6: Integration

### 6.1 Makefile

```makefile
PERL = perl
PROVE = prove

.PHONY: test install lint clean

test:
	$(PROVE) -l t/

install:
	install -m 755 bin/threads $(HOME)/.local/bin/threads-perl
	cp -r lib/Threads $(PERL_LIB)/

lint:
	perlcritic --stern lib/ bin/

clean:
	rm -rf blib _build
```

### 6.2 cpanfile

```perl
requires 'YAML::Tiny', '1.70';

on 'test' => sub {
    requires 'Test::More', '0.98';
};
```

### 6.3 Installation

```bash
# Install dependencies
cpanm --installdeps .

# Run tests
make test

# Install (symlink for development)
ln -s $PWD/bin/threads ~/.local/bin/threads-perl
```

---

## Implementation Order

### Week 1: Foundation
1. [ ] Set up directory structure and cpanfile
2. [ ] Implement Workspace.pm (path resolution, thread discovery)
3. [ ] Implement Section.pm (get/set/append)
4. [ ] Implement Thread.pm (parse, serialize, accessors)
5. [ ] Write unit tests for above

### Week 2: Read Commands
6. [ ] Implement CLI.pm skeleton with dispatch
7. [ ] Implement `read` command
8. [ ] Implement `list` command (with all flags)
9. [ ] Implement `stats` command
10. [ ] Implement `validate` command

### Week 3: Write Commands
11. [ ] Implement `new` command
12. [ ] Implement `body` command
13. [ ] Implement `note` command (add/edit/remove)
14. [ ] Implement `todo` command (add/check/uncheck/remove)
15. [ ] Implement `log` command

### Week 4: Lifecycle & Git
16. [ ] Implement `status`, `resolve`, `reopen` commands
17. [ ] Implement `update`, `remove`, `move` commands
18. [ ] Implement Git.pm
19. [ ] Implement `commit` command
20. [ ] Add `--commit` flag support to all write commands

### Week 5: Polish
21. [ ] Integration tests
22. [ ] Performance benchmarking vs bash/go/python
23. [ ] Documentation (--help output)
24. [ ] Code review and cleanup

---

## Key Implementation Notes

### Perl-Specific Advantages

1. **Native regex**: No external tools for text manipulation
   ```perl
   # Bash needs: echo "$content" | gawk '...'
   # Perl: $content =~ s/pattern/replacement/;
   ```

2. **Here-docs**: Clean multiline strings
   ```perl
   my $template = <<'END';
   ---
   id: %s
   name: %s
   ---
   END
   ```

3. **Hash slices**: Elegant option handling
   ```perl
   my %defaults = (status => 'idea', desc => '');
   my %opts = (%defaults, @args);
   ```

### Gotchas to Avoid

1. **YAML::Tiny limitations**: No anchors/aliases, no complex types. Fine for frontmatter.

2. **Regex greediness**: Use `.*?` (non-greedy) for section matching.

3. **File encoding**: Always use `:encoding(UTF-8)` for file handles.

4. **Exit codes**: `die` sets `$?` to 255; use explicit `exit` for controlled codes.

5. **Glob expansion**: Perl's `glob()` doesn't expand `**`; use explicit patterns or File::Glob.

### Testing Against Bash Version

Run both implementations on same test fixtures:

```bash
# Create identical test workspace
export WORKSPACE=/tmp/threads-test

# Run bash version
threads-bash list

# Run perl version
threads-perl list

# Compare output
diff <(threads-bash list) <(threads-perl list)
```

---

## Success Metrics

From the thread's decision criteria:

| Metric | Target | How to Measure |
|--------|--------|----------------|
| Lines of code | <800 (25%+ less than Python) | `wc -l lib/**/*.pm bin/threads` |
| Startup time | <15ms | `hyperfine 'threads-perl --version'` |
| Readability | Accessible to non-Perl devs | Code review |
| Correctness | Pass all bash test cases | `make test` |

---

## Open Questions

1. **JSON output**: Use JSON::PP (core) or JSON::XS (faster but XS)?
   - Recommendation: JSON::PP for zero-dependency goal

2. **Color output**: Use Term::ANSIColor or raw escape codes?
   - Recommendation: Raw codes to avoid dependency

3. **Bless vs hash**: Pure hash refs or blessed objects?
   - Recommendation: Blessed for method dispatch, but minimal OO

4. **Error messages**: Match bash exactly or improve?
   - Recommendation: Match semantics, allow style improvements
