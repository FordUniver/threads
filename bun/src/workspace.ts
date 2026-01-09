import * as fs from 'fs';
import * as path from 'path';
import { extractIDFromPath, extractNameFromPath, Thread } from './thread';

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
  // Handle explicit "." for workspace level
  if (targetPath === '.') {
    return {
      threadsDir: path.join(ws, '.threads'),
      category: '-',
      project: '-',
      levelDesc: 'workspace-level thread',
    };
  }

  let absPath: string;

  // Resolve to absolute path
  if (path.isAbsolute(targetPath)) {
    absPath = targetPath;
  } else {
    const wsRelPath = path.join(ws, targetPath);
    if (fs.existsSync(wsRelPath) && fs.statSync(wsRelPath).isDirectory()) {
      absPath = wsRelPath;
    } else if (fs.existsSync(targetPath) && fs.statSync(targetPath).isDirectory()) {
      absPath = path.resolve(targetPath);
    } else {
      throw new Error(`path not found: ${targetPath}`);
    }
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
