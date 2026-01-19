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

// Stage files, skipping any that don't exist (assumed to be already-staged deletions)
export function add(ws: string, ...files: string[]): void {
  const existingFiles: string[] = [];
  for (const f of files) {
    const fullPath = path.isAbsolute(f) ? f : path.join(ws, f);
    if (fs.existsSync(fullPath)) {
      existingFiles.push(f);
    }
    // Non-existent files are assumed to be deletions already staged
  }

  if (existingFiles.length === 0) {
    return;
  }

  const result = runGit(ws, ['add', ...existingFiles]);
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

// Auto commit a file (push is user's responsibility)
export function autoCommit(ws: string, file: string, message: string): void {
  const relPath = path.relative(ws, file);

  commit(ws, [relPath], message);
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

// Check if a path looks like a thread file (.threads/*.md)
function isThreadPath(p: string): boolean {
  return p.includes('.threads/') && p.endsWith('.md');
}

// Find deleted thread files that are staged or in working tree
export function findDeletedThreadFiles(ws: string): string[] {
  const result = runGit(ws, ['status', '--porcelain']);
  if (!result.success) {
    return [];
  }

  const deleted: string[] = [];
  const lines = result.output.split('\n');
  for (const line of lines) {
    if (line.length < 4) {
      continue;
    }
    // Porcelain format: XY PATH
    // X = index status, Y = worktree status
    // D in either position means deleted
    const indexStatus = line[0];
    const worktreeStatus = line[1];
    const filePath = line.substring(3);

    if ((indexStatus === 'D' || worktreeStatus === 'D') && isThreadPath(filePath)) {
      deleted.push(path.join(ws, filePath));
    }
  }

  return deleted;
}
