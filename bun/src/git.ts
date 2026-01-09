import * as path from 'path';
import * as fs from 'fs';

// Run a git command and return success/failure
function runGit(ws: string, args: string[]): { success: boolean; output: string } {
  const proc = Bun.spawnSync(['git', '-C', ws, ...args], {
    stdout: 'pipe',
    stderr: 'pipe',
  });
  return {
    success: proc.exitCode === 0,
    output: proc.stdout.toString() + proc.stderr.toString(),
  };
}

// Check if a file has uncommitted changes
export function hasChanges(ws: string, relPath: string): boolean {
  // Check unstaged changes
  let result = runGit(ws, ['diff', '--quiet', '--', relPath]);
  if (!result.success) {
    return true;
  }

  // Check staged changes
  result = runGit(ws, ['diff', '--cached', '--quiet', '--', relPath]);
  if (!result.success) {
    return true;
  }

  // Check if untracked
  if (!isTracked(ws, relPath)) {
    return true;
  }

  return false;
}

// Check if a file is tracked by git
export function isTracked(ws: string, relPath: string): boolean {
  const result = runGit(ws, ['ls-files', '--error-unmatch', relPath]);
  return result.success;
}

// Check if a file exists in HEAD
export function existsInHEAD(ws: string, relPath: string): boolean {
  const ref = `HEAD:${relPath}`;
  const result = runGit(ws, ['cat-file', '-e', ref]);
  return result.success;
}

// Stage a file
export function add(ws: string, ...files: string[]): void {
  const result = runGit(ws, ['add', ...files]);
  if (!result.success) {
    throw new Error(`git add failed: ${result.output}`);
  }
}

// Create a commit with the given message
export function commit(ws: string, files: string[], message: string): void {
  // Stage files
  add(ws, ...files);

  // Commit
  const result = runGit(ws, ['commit', '-m', message, ...files]);
  if (!result.success) {
    throw new Error(`git commit failed: ${result.output}`);
  }
}

// Pull with rebase and push
export function push(ws: string): void {
  // Pull with rebase
  let result = runGit(ws, ['pull', '--rebase']);
  if (!result.success) {
    throw new Error(`git pull --rebase failed: ${result.output}`);
  }

  // Push
  result = runGit(ws, ['push']);
  if (!result.success) {
    throw new Error(`git push failed: ${result.output}`);
  }
}

// Auto commit and push a file
export function autoCommit(ws: string, file: string, message: string): void {
  const relPath = path.relative(ws, file);

  commit(ws, [relPath], message);

  try {
    push(ws);
  } catch (e) {
    // Warning only - commit succeeded
    console.log(`WARNING: git push failed (commit succeeded): ${e}`);
  }
}

// Generate conventional commit message for thread changes
export function generateCommitMessage(ws: string, files: string[]): string {
  const added: string[] = [];
  const modified: string[] = [];
  const deleted: string[] = [];

  for (const file of files) {
    const relPath = path.relative(ws, file);
    let name = path.basename(file);
    name = name.replace(/\.md$/, '');

    if (existsInHEAD(ws, relPath)) {
      // File exists in HEAD
      if (fs.existsSync(file)) {
        modified.push(name);
      } else {
        deleted.push(name);
      }
    } else {
      // File not in HEAD - it's new
      added.push(name);
    }
  }

  const total = added.length + modified.length + deleted.length;

  if (total === 1) {
    if (added.length === 1) {
      return `threads: add ${extractID(added[0])}`;
    }
    if (modified.length === 1) {
      return `threads: update ${extractID(modified[0])}`;
    }
    return `threads: remove ${extractID(deleted[0])}`;
  }

  if (total <= 3) {
    const ids = [...added, ...modified, ...deleted].map(extractID);
    let action = 'update';
    if (added.length === total) {
      action = 'add';
    } else if (deleted.length === total) {
      action = 'remove';
    }
    return `threads: ${action} ${ids.join(' ')}`;
  }

  let action = 'update';
  if (added.length === total) {
    action = 'add';
  } else if (deleted.length === total) {
    action = 'remove';
  }
  return `threads: ${action} ${total} threads`;
}

// Extract ID prefix from filename
function extractID(name: string): string {
  if (name.length >= 6 && isHex(name.substring(0, 6))) {
    return name.substring(0, 6);
  }
  return name;
}

function isHex(s: string): boolean {
  return /^[0-9a-f]+$/.test(s);
}
