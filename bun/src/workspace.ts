import * as fs from 'fs';
import * as path from 'path';
import { spawnSync } from 'child_process';
import { extractIDFromPath, extractNameFromPath, Thread } from './thread';

// FindOptions contains options for finding threads with direction and boundary controls.
export interface FindOptions {
  // down specifies subdirectory search depth. undefined = no recursion, 0 = unlimited, N = N levels
  down?: number;
  // up specifies parent directory search depth. undefined = no up search, 0 = unlimited (to git root), N = N levels
  up?: number;
}

// Check if a directory is a git root (contains .git directory or file for worktrees)
function isGitRoot(dir: string): boolean {
  return fs.existsSync(path.join(dir, '.git'));
}

// Find the git root for the current directory
export function findGitRoot(): string {
  const result = spawnSync('git', ['rev-parse', '--show-toplevel'], {
    encoding: 'utf-8',
  });
  if (result.status !== 0) {
    throw new Error('not in a git repository. threads requires a git repo to define scope');
  }
  return result.stdout.trim();
}

// Collect threads at a specific path (from its .threads directory)
function collectThreadsAtPath(dir: string, threads: string[]): void {
  const threadsDir = path.join(dir, '.threads');
  if (!fs.existsSync(threadsDir)) {
    return;
  }
  try {
    const stat = fs.statSync(threadsDir);
    if (!stat.isDirectory()) {
      return;
    }
    const entries = fs.readdirSync(threadsDir);
    for (const entry of entries) {
      if (entry.endsWith('.md')) {
        const fullPath = path.join(threadsDir, entry);
        // Skip archive subdirectory
        if (!fullPath.includes('/archive/')) {
          threads.push(fullPath);
        }
      }
    }
  } catch {
    // Ignore errors
  }
}

// Find threads going down into subdirectories
function findThreadsDown(
  dir: string,
  gitRoot: string,
  threads: string[],
  currentDepth: number,
  maxDepth: number,
  crossGitBoundaries: boolean
): void {
  // Check depth limit (-1 means unlimited)
  if (maxDepth >= 0 && currentDepth >= maxDepth) {
    return;
  }

  let entries: fs.Dirent[];
  try {
    entries = fs.readdirSync(dir, { withFileTypes: true });
  } catch {
    return;
  }

  for (const entry of entries) {
    if (!entry.isDirectory()) {
      continue;
    }

    const name = entry.name;

    // Skip hidden directories
    if (name.startsWith('.')) {
      continue;
    }

    const subdir = path.join(dir, name);

    // Check git boundary
    if (!crossGitBoundaries && subdir !== gitRoot && isGitRoot(subdir)) {
      continue;
    }

    // Collect threads at this level
    collectThreadsAtPath(subdir, threads);

    // Continue recursing
    findThreadsDown(subdir, gitRoot, threads, currentDepth + 1, maxDepth, crossGitBoundaries);
  }
}

// Find threads going up into parent directories
function findThreadsUp(
  dir: string,
  gitRoot: string,
  threads: string[],
  currentDepth: number,
  maxDepth: number,
  crossGitBoundaries: boolean
): void {
  // Check depth limit (-1 means unlimited)
  if (maxDepth >= 0 && currentDepth >= maxDepth) {
    return;
  }

  const parent = path.dirname(dir);
  if (parent === dir) {
    return; // reached filesystem root
  }

  const absParent = path.resolve(parent);
  const absGitRoot = path.resolve(gitRoot);

  // Check git boundary: stop at git root unless crossing is allowed
  if (!crossGitBoundaries && !absParent.startsWith(absGitRoot)) {
    return;
  }

  // Collect threads at parent
  collectThreadsAtPath(absParent, threads);

  // Continue up
  findThreadsUp(absParent, gitRoot, threads, currentDepth + 1, maxDepth, crossGitBoundaries);
}

// Find threads with direction and boundary controls.
// This is the primary search function supporting --up, --down, and boundary flags.
export function findThreadsWithOptions(startPath: string, gitRoot: string, options: FindOptions): string[] {
  const threads: string[] = [];

  const absStart = path.resolve(startPath);

  // Always collect threads at start_path
  collectThreadsAtPath(absStart, threads);

  // Search down (subdirectories)
  if (options.down !== undefined) {
    // Convert: 0 = unlimited (-1 internally), N > 0 = N levels
    const maxDepth = options.down === 0 ? -1 : options.down;
    findThreadsDown(absStart, gitRoot, threads, 0, maxDepth, false);
  }

  // Search up (parent directories)
  if (options.up !== undefined) {
    // Convert: 0 = unlimited (-1 internally), N > 0 = N levels
    const maxDepth = options.up === 0 ? -1 : options.up;
    findThreadsUp(absStart, gitRoot, threads, 0, maxDepth, false);
  }

  // Sort and deduplicate
  threads.sort();
  const deduplicated: string[] = [];
  for (let i = 0; i < threads.length; i++) {
    if (i === 0 || threads[i] !== threads[i - 1]) {
      deduplicated.push(threads[i]);
    }
  }

  return deduplicated;
}

// Secure path containment check (prevents path traversal attacks)
function isUnderWorkspace(filePath: string, workspace: string): boolean {
  const relative = path.relative(workspace, filePath);
  return !relative.startsWith('..') && !path.isAbsolute(relative);
}

// Find workspace root from $WORKSPACE environment variable
export function findWorkspace(): string {
  const ws = process.env.WORKSPACE || '';
  if (!ws) {
    throw new Error('WORKSPACE environment variable not set');
  }
  // Normalize path to handle double slashes, trailing slashes, etc.
  const normalized = path.resolve(ws);
  if (!fs.existsSync(normalized) || !fs.statSync(normalized).isDirectory()) {
    throw new Error(`WORKSPACE directory does not exist: ${ws}`);
  }
  return normalized;
}

// Helper to get .md files from a directory
function getMdFiles(dir: string): string[] {
  if (!fs.existsSync(dir)) {
    return [];
  }
  try {
    return fs.readdirSync(dir)
      .filter(f => f.endsWith('.md'))
      .map(f => path.join(dir, f));
  } catch {
    return [];
  }
}

// Find all thread files in the workspace
export function findAllThreads(ws: string): string[] {
  const threads: string[] = [];

  // Workspace-level threads
  threads.push(...getMdFiles(path.join(ws, '.threads')));

  // Category-level threads (ws/*/. threads/*.md)
  try {
    const entries = fs.readdirSync(ws, { withFileTypes: true });
    for (const entry of entries) {
      if (entry.isDirectory() && !entry.name.startsWith('.')) {
        const catDir = path.join(ws, entry.name);
        threads.push(...getMdFiles(path.join(catDir, '.threads')));

        // Project-level threads (ws/*/*/.threads/*.md)
        try {
          const catEntries = fs.readdirSync(catDir, { withFileTypes: true });
          for (const projEntry of catEntries) {
            if (projEntry.isDirectory() && !projEntry.name.startsWith('.')) {
              const projDir = path.join(catDir, projEntry.name);
              threads.push(...getMdFiles(path.join(projDir, '.threads')));
            }
          }
        } catch {
          // Ignore errors reading category subdirs
        }
      }
    }
  } catch {
    // Ignore errors reading workspace dir
  }

  // Filter out archive directories and sort
  const filtered = threads.filter(t => !t.includes('/archive/'));
  filtered.sort();
  return filtered;
}

// Scope represents thread placement information
export interface Scope {
  threadsDir: string;
  category: string;
  project: string;
  levelDesc: string;
}

// Infer scope from a path
export function inferScope(ws: string, targetPath: string): Scope {
  let absPath: string;

  // Handle "." as PWD (current directory), not workspace root
  if (targetPath === '.') {
    absPath = process.cwd();
  } else if (path.isAbsolute(targetPath)) {
    absPath = targetPath;
  } else if (targetPath.startsWith('./')) {
    // PWD-relative path
    absPath = path.resolve(process.cwd(), targetPath);
  } else {
    // Git-root-relative path: try workspace first, then PWD
    const wsRelPath = path.join(ws, targetPath);
    if (fs.existsSync(wsRelPath) && fs.statSync(wsRelPath).isDirectory()) {
      absPath = wsRelPath;
    } else if (fs.existsSync(targetPath) && fs.statSync(targetPath).isDirectory()) {
      absPath = path.resolve(targetPath);
    } else {
      throw new Error(`path not found: ${targetPath}`);
    }
  }

  // Verify path exists
  if (!fs.existsSync(absPath) || !fs.statSync(absPath).isDirectory()) {
    throw new Error(`path not found or not a directory: ${targetPath}`);
  }

  // Must be within workspace (use relative path check for security)
  if (!isUnderWorkspace(absPath, ws)) {
    return {
      threadsDir: path.join(ws, '.threads'),
      category: '-',
      project: '-',
      levelDesc: 'workspace-level thread',
    };
  }

  let rel = absPath.substring(ws.length);
  if (rel.startsWith('/')) {
    rel = rel.substring(1);
  }

  if (rel === '') {
    return {
      threadsDir: path.join(ws, '.threads'),
      category: '-',
      project: '-',
      levelDesc: 'workspace-level thread',
    };
  }

  const parts = rel.split('/').slice(0, 3);
  const category = parts[0];
  let project = '-';

  if (parts.length >= 2 && parts[1] !== '') {
    project = parts[1];
  }

  if (project === '-') {
    return {
      threadsDir: path.join(ws, category, '.threads'),
      category,
      project: '-',
      levelDesc: `category-level thread (${category})`,
    };
  }

  return {
    threadsDir: path.join(ws, category, project, '.threads'),
    category,
    project,
    levelDesc: `project-level thread (${category}/${project})`,
  };
}

// Parse thread path to extract category, project, name
export function parseThreadPath(ws: string, threadPath: string): { category: string; project: string; name: string } {
  let rel = threadPath.startsWith(ws) ? threadPath.substring(ws.length) : threadPath;
  if (rel.startsWith('/')) {
    rel = rel.substring(1);
  }

  const name = extractNameFromPath(threadPath);

  // Check if workspace-level
  if (rel.startsWith('.threads/')) {
    return { category: '-', project: '-', name };
  }

  // Extract category and project from path like: category/project/.threads/name.md
  const parts = rel.split('/');
  let category = '-';
  let project = '-';

  if (parts.length >= 2) {
    category = parts[0];
    if (parts[1] === '.threads') {
      project = '-';
    } else if (parts.length >= 3) {
      project = parts[1];
    }
  }

  return { category, project, name };
}

// Compute git-relative path for a thread file.
// Returns the directory containing .threads relative to git root.
export function computeRelativePath(ws: string, threadPath: string): string {
  // Get the directory containing .threads
  const threadsDir = path.dirname(threadPath);  // Remove filename
  const parentDir = path.dirname(threadsDir);   // Remove .threads

  // If parentDir equals workspace root, return "."
  const wsReal = path.resolve(ws);
  const parentReal = path.resolve(parentDir);

  if (parentReal === wsReal) {
    return '.';
  }

  // Compute relative path from workspace root
  if (parentReal.startsWith(wsReal)) {
    let rel = parentReal.substring(wsReal.length);
    if (rel.startsWith('/')) {
      rel = rel.substring(1);
    }
    return rel || '.';
  }

  return '.';
}

// Generate unique 6-character hex ID
export function generateID(ws: string): string {
  const existing = new Set<string>();

  const threads = findAllThreads(ws);
  for (const t of threads) {
    const id = extractIDFromPath(t);
    if (id) {
      existing.add(id);
    }
  }

  // Try to generate unique ID
  for (let i = 0; i < 10; i++) {
    const bytes = new Uint8Array(3);
    crypto.getRandomValues(bytes);
    const id = Array.from(bytes)
      .map(b => b.toString(16).padStart(2, '0'))
      .join('');
    if (!existing.has(id)) {
      return id;
    }
  }

  throw new Error('could not generate unique ID after 10 attempts');
}

// Slugify a title to kebab-case
export function slugify(title: string): string {
  let s = title.toLowerCase();
  // Replace non-alphanumeric with hyphens
  s = s.replace(/[^a-z0-9]+/g, '-');
  // Clean up multiple hyphens
  s = s.replace(/-+/g, '-');
  // Trim leading/trailing hyphens
  s = s.replace(/^-|-$/g, '');
  return s;
}

// Find thread by ID or name reference
export function findByRef(ws: string, ref: string): string {
  const threads = findAllThreads(ws);

  // Fast path: exact 6-char hex ID match
  const idRe = /^[0-9a-f]{6}$/;
  if (idRe.test(ref)) {
    for (const t of threads) {
      if (extractIDFromPath(t) === ref) {
        return t;
      }
    }
  }

  // Slow path: name matching
  const substringMatches: string[] = [];
  const refLower = ref.toLowerCase();

  for (const t of threads) {
    const name = extractNameFromPath(t);

    // Exact name match
    if (name === ref) {
      return t;
    }

    // Substring match (case-insensitive)
    if (name.toLowerCase().includes(refLower)) {
      substringMatches.push(t);
    }
  }

  if (substringMatches.length === 1) {
    return substringMatches[0];
  }

  if (substringMatches.length > 1) {
    const ids = substringMatches.map(m => {
      const id = extractIDFromPath(m);
      const name = extractNameFromPath(m);
      return `${id} (${name})`;
    });
    throw new Error(`ambiguous reference '${ref}' matches ${substringMatches.length} threads: ${ids.join(', ')}`);
  }

  throw new Error(`thread not found: ${ref}`);
}
