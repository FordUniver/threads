import * as yaml from 'js-yaml';
import * as fs from 'fs';
import * as path from 'path';

// Status constants
export const TERMINAL_STATUSES = ['resolved', 'superseded', 'deferred'];
export const ACTIVE_STATUSES = ['idea', 'planning', 'active', 'blocked', 'paused'];
export const ALL_STATUSES = [...ACTIVE_STATUSES, ...TERMINAL_STATUSES];

// Frontmatter interface
export interface Frontmatter {
  id: string;
  name: string;
  desc: string;
  status: string;
}

// Thread class
export class Thread {
  path: string;
  frontmatter: Frontmatter;
  content: string;
  bodyStart: number;

  constructor(filePath: string) {
    this.path = filePath;
    this.frontmatter = { id: '', name: '', desc: '', status: '' };
    this.content = '';
    this.bodyStart = 0;
  }

  static parse(filePath: string): Thread {
    const t = new Thread(filePath);
    t.content = fs.readFileSync(filePath, 'utf-8');
    t.parseFrontmatter();

    // Extract ID from filename if not in frontmatter
    if (!t.frontmatter.id) {
      t.frontmatter.id = extractIDFromPath(filePath);
    }

    return t;
  }

  parseFrontmatter(): void {
    if (!this.content.startsWith('---\n')) {
      throw new Error('missing frontmatter delimiter');
    }

    const end = this.content.indexOf('\n---', 4);
    if (end === -1) {
      throw new Error('unclosed frontmatter');
    }

    const yamlContent = this.content.substring(4, end);
    this.bodyStart = end + 4; // skip opening ---, yaml, closing ---, and newline

    const parsed = yaml.load(yamlContent) as Partial<Frontmatter>;
    this.frontmatter = {
      id: parsed?.id || '',
      name: parsed?.name || '',
      desc: parsed?.desc || '',
      status: parsed?.status || '',
    };
  }

  get id(): string {
    return this.frontmatter.id;
  }

  get name(): string {
    return this.frontmatter.name;
  }

  get status(): string {
    return this.frontmatter.status;
  }

  baseStatus(): string {
    return baseStatus(this.frontmatter.status);
  }

  body(): string {
    if (this.bodyStart >= this.content.length) {
      return '';
    }
    return this.content.substring(this.bodyStart);
  }

  setFrontmatterField(field: string, value: string): void {
    switch (field) {
      case 'id':
        this.frontmatter.id = value;
        break;
      case 'name':
        this.frontmatter.name = value;
        break;
      case 'desc':
        this.frontmatter.desc = value;
        break;
      case 'status':
        this.frontmatter.status = value;
        break;
      default:
        throw new Error(`unknown field: ${field}`);
    }
    this.rebuildContent();
  }

  rebuildContent(): void {
    let sb = '---\n';

    // Serialize frontmatter in specific order
    const fm: Record<string, string> = {};
    if (this.frontmatter.id) fm.id = this.frontmatter.id;
    if (this.frontmatter.name) fm.name = this.frontmatter.name;
    if (this.frontmatter.desc !== undefined) fm.desc = this.frontmatter.desc;
    if (this.frontmatter.status) fm.status = this.frontmatter.status;

    sb += yaml.dump(fm, { lineWidth: -1 });
    sb += '---\n';

    // Preserve body
    if (this.bodyStart < this.content.length) {
      sb += this.content.substring(this.bodyStart);
    }

    this.content = sb;
  }

  write(): void {
    fs.writeFileSync(this.path, this.content);
  }

  relPath(ws: string): string {
    return path.relative(ws, this.path);
  }
}

// ID prefix regex
const idPrefixRe = /^([0-9a-f]{6})-/;

// Extract 6-char hex ID from filename
export function extractIDFromPath(filePath: string): string {
  const filename = path.basename(filePath);
  const nameWithoutExt = filename.replace(/\.md$/, '');

  const match = nameWithoutExt.match(idPrefixRe);
  if (match && match[1]) {
    return match[1];
  }
  return '';
}

// Extract human-readable name from filename
export function extractNameFromPath(filePath: string): string {
  const filename = path.basename(filePath);
  const nameWithoutExt = filename.replace(/\.md$/, '');

  const match = nameWithoutExt.match(idPrefixRe);
  if (match) {
    return nameWithoutExt.substring(7); // skip "abc123-"
  }
  return nameWithoutExt;
}

// Strip reason suffix from status
export function baseStatus(status: string): string {
  const idx = status.indexOf(' (');
  if (idx !== -1) {
    return status.substring(0, idx);
  }
  return status;
}

// Check if status is terminal
export function isTerminal(status: string): boolean {
  const base = baseStatus(status);
  return TERMINAL_STATUSES.includes(base);
}

// Check if status is valid
export function isValidStatus(status: string): boolean {
  const base = baseStatus(status);
  return ALL_STATUSES.includes(base);
}
