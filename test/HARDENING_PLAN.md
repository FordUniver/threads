# Test Harness Hardening Plan

## Problem Statement

Tests exhibit flaky behavior: passing when run together but failing when run individually, or passing in fresh bash but failing in current shell. This makes analysis impossible and indicates fundamental isolation issues.

## Root Cause Analysis

### Issue 1: Environment Pollution (CRITICAL)

**Current behavior:**
```bash
# In run_tests.sh, line 102:
source "$test_file" || true
```

If `WORKSPACE` is set in the parent shell, it persists into the test runner. The `reset_counters()` function does `unset WORKSPACE`, but this happens too late—after environment is already polluted.

**Evidence:** Tests pass in `bash -c './run_tests.sh'` but fail when WORKSPACE is exported.

**Fix:** Unset ALL known variables at script start, before any test file is sourced.

### Issue 2: Tests Sourced, Not Subshelled

**Current behavior:** Test files are `source`d directly into the runner's process.

**Problems:**
- Function definitions accumulate across test files
- Variable leaks between tests
- If a test crashes without cleanup, state leaks to subsequent tests
- No process isolation

**Fix:** Run each test file in a clean subshell with explicit environment.

### Issue 3: No Isolation Validation

**Current behavior:** No way to verify tests work both together AND individually.

**Problem:** A test can accidentally depend on state from a prior test without detection.

**Fix:** Add `--validate` mode that runs tests both together and individually, comparing results.

### Issue 4: Perl Strict WORKSPACE Requirement

**Current behavior:** Perl implementation dies if WORKSPACE is unset (Workspace.pm line 22).

**Problem:** If test harness fails to set WORKSPACE, Perl fails differently than other implementations.

**Status:** Already fixed to warn and fallback.

## Detailed Fixes

### Fix 1: Environment Sanitization

Add at start of `run_tests.sh` (before any sourcing):

```bash
# Sanitize environment - prevent parent shell pollution
unset WORKSPACE TEST_WS _ORIGINAL_PWD 2>/dev/null || true
unset _TEST_PASSED _TEST_FAILED _TEST_CURRENT 2>/dev/null || true

# Ensure clean HOME-relative paths don't leak
export HOME="${HOME:-/tmp}"
```

### Fix 2: Subshell Execution

Replace sourcing with subshell execution:

```bash
# OLD (line 102):
source "$test_file" || true

# NEW:
(
    # Fresh environment for this test file
    export THREADS_BIN SCRIPT_DIR REPO_DIR
    unset WORKSPACE TEST_WS _ORIGINAL_PWD

    # Source libraries in subshell
    source "$SCRIPT_DIR/lib/assertions.sh"
    source "$SCRIPT_DIR/lib/setup.sh"
    source "$SCRIPT_DIR/lib/helpers.sh"

    # Source and run test file
    source "$test_file"

    # Output results for parent to parse
    echo "RESULTS:$_TEST_PASSED:$_TEST_FAILED"
) 2>&1 | while IFS=: read -r tag passed failed; do
    if [[ "$tag" == "RESULTS" ]]; then
        TOTAL_PASSED=$((TOTAL_PASSED + passed))
        TOTAL_FAILED=$((TOTAL_FAILED + failed))
    else
        # Pass through test output
        echo "$tag${passed:+:$passed}${failed:+:$failed}"
    fi
done
```

**Simpler alternative:** Run each test file as a separate process:

```bash
# Execute test file as subprocess
result=$(bash "$SCRIPT_DIR/run_single_test.sh" "$test_file" "$THREADS_BIN")
```

### Fix 3: Validation Mode

Add `--validate` flag to run tests both ways and compare:

```bash
if [[ "${VALIDATE:-}" == "1" ]]; then
    echo "Running validation mode..."

    # Run all tests together
    ./run_tests.sh "$THREADS_BIN" > /tmp/together.out 2>&1
    together_exit=$?

    # Run each test file individually
    individual_exit=0
    for tf in cases/test_*.sh; do
        if ! ./run_tests.sh "$THREADS_BIN" "$tf" > /dev/null 2>&1; then
            echo "FAIL (individual): $tf"
            individual_exit=1
        fi
    done

    if [[ $together_exit -ne $individual_exit ]]; then
        echo "ISOLATION ERROR: Tests pass together but not individually (or vice versa)"
        exit 1
    fi
fi
```

### Fix 4: Setup Assertions

Add validation in `setup_test_workspace()`:

```bash
setup_test_workspace() {
    # Fail fast if WORKSPACE was already set (indicates pollution)
    if [[ -n "${WORKSPACE:-}" && ! -d "${TEST_WS:-}" ]]; then
        echo "ERROR: WORKSPACE already set before setup - environment pollution" >&2
        exit 1
    fi

    _ORIGINAL_PWD="$PWD"
    TEST_WS=$(mktemp -d "${TMPDIR:-/tmp}/threads-test.XXXXXX")
    # ... rest of setup
}
```

## Benchmark Extensions

### Current Coverage

Only 3 operations benchmarked:
1. `--help` (cold start)
2. `list` (50 threads)
3. `list -r` (recursive)

### Missing Coverage

| Operation | Why Important |
|-----------|---------------|
| `new` | Creation overhead, ID generation |
| `read <id>` | Single-file parsing |
| `status <id> <value>` | Mutation performance |
| `note add <id> "text"` | Section manipulation |
| `todo add <id> "text"` | Section manipulation |
| `list` with 500 threads | Scale testing |
| `list` with 1000 threads | Stress testing |
| `stats` | Aggregation performance |
| `validate` | Full workspace scan |

### Proposed benchmark.sh Additions

```bash
# After creating 50 threads, also create a single thread for mutation tests
SINGLE_ID="aaaaaa"
create_test_thread "$SINGLE_ID" "Benchmark Thread" "active"

# Benchmark creation (creates and removes)
echo "### new (thread creation)"
for name in "${!IMPLS[@]}"; do
    cmd="${IMPL_CMDS[$name]}"
    # Create temp threads, measure time, clean up
    benchmark_create "$name" "$cmd"
done

# Benchmark single read
echo "### read (single thread)"
benchmark_command "${name}_read" "$cmd" "read $SINGLE_ID"

# Benchmark mutation
echo "### status (change status)"
benchmark_command "${name}_status" "$cmd" "status $SINGLE_ID active"

# Scale tests
echo "### list with 500 threads"
create_n_threads 500
benchmark_command "${name}_list_500" "$cmd" "list"
```

### Memory Profiling

With hyperfine 1.19+:
```bash
hyperfine --show-output "$cmd list"
```

Or with GNU time:
```bash
/usr/bin/time -v $cmd list 2>&1 | grep "Maximum resident"
```

## Extended Test Cases

### Current: 42 tests across 9 files

| File | Tests | Coverage |
|------|-------|----------|
| test_new.sh | 7 | Thread creation |
| test_list.sh | 8 | Listing/filtering |
| test_read.sh | 3 | Reading threads |
| test_note.sh | 3 | Note operations |
| test_body.sh | 4 | Body section |
| test_todo.sh | 5 | Todo operations |
| test_log.sh | 3 | Log entries |
| test_lifecycle.sh | 5 | Status transitions |
| test_edge_cases.sh | 4 | Historical bugs |

### Proposed Additions

**test_concurrent.sh** - Race conditions
```bash
test_concurrent_creates() {
    # Create 10 threads in parallel, verify all created
}

test_concurrent_updates() {
    # Update same thread from multiple processes
}
```

**test_performance.sh** - Performance regression
```bash
test_list_under_100ms() {
    # With 50 threads, list should complete under 100ms
    # (Shell excluded from this test)
}
```

**test_error_handling.sh** - Error paths
```bash
test_invalid_id_format() {
    # Non-hex IDs should fail gracefully
}

test_missing_workspace() {
    # Unset WORKSPACE, verify graceful error
}

test_readonly_workspace() {
    # chmod 444 on .threads, verify error message
}
```

**test_git.sh** - Git integration
```bash
test_commit_creates_commit() {
    # After threads new --commit, git log should show commit
}

test_commit_message_format() {
    # Commit message should contain thread name
}
```

## Implementation Order

1. **Phase 1: Critical fixes** (blocks all further work)
   - [ ] Unset WORKSPACE at script start
   - [ ] Add subshell execution for test files
   - [ ] Add --validate mode

2. **Phase 2: Benchmark extensions**
   - [ ] Add creation/mutation benchmarks
   - [ ] Add scale tests (500, 1000 threads)
   - [ ] Add memory profiling

3. **Phase 3: Test extensions**
   - [ ] Add error handling tests
   - [ ] Add performance regression tests
   - [ ] Add git integration tests

4. **Phase 4: CI/automation**
   - [ ] Add Makefile targets: test, test-all, test-validate, benchmark
   - [ ] Add GitHub Actions / GitLab CI workflow

## Makefile Targets

```makefile
.PHONY: test test-all test-validate benchmark

# Test shell (default) implementation
test:
	./test/run_tests.sh

# Test all implementations
test-all:
	@echo "Testing Shell..."
	./test/run_tests.sh ./shell/threads
	@echo "Testing Go..."
	./test/run_tests.sh ./go/threads
	@echo "Testing Python..."
	./test/run_tests.sh "uv run --quiet --directory ./python python -m threads"
	@echo "Testing Perl..."
	./test/run_tests.sh "perl -I./perl/lib ./perl/bin/threads"

# Validate isolation (tests pass both together and individually)
test-validate:
	VALIDATE=1 ./test/run_tests.sh

# Run benchmarks
benchmark:
	./test/benchmark.sh
```

## Verification

After implementing fixes:

```bash
# 1. Verify tests pass in current shell
./test/run_tests.sh

# 2. Verify tests pass in fresh shell
bash -c './test/run_tests.sh'

# 3. Verify tests pass with WORKSPACE set
WORKSPACE=/tmp/fake ./test/run_tests.sh

# 4. Verify individual test files pass
for f in test/cases/test_*.sh; do
    ./test/run_tests.sh "$f" || echo "FAIL: $f"
done

# 5. Verify all implementations pass
make test-all

# 6. Run validation mode
make test-validate
```

## Success Criteria

1. Tests pass regardless of parent shell environment
2. Tests pass when run individually or together
3. All 4 implementations pass the same test suite
4. Benchmarks cover creation, reading, mutation, and scale
5. CI can catch regressions automatically
