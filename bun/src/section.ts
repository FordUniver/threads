import * as crypto from 'crypto';
import { formatDate, formatTime } from './utils';

// Extract content of a section (between ## Name and next ## or EOF)
export function extractSection(content: string, name: string): string {
  const escapedName = name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const pattern = new RegExp(`^## ${escapedName}\\n([\\s\\S]+?)(?=^## |$)`, 'm');
  const match = content.match(pattern);
  if (!match || !match[1]) {
    return '';
  }
  return match[1].trim();
}

// Replace content of a section
export function replaceSection(content: string, name: string, newContent: string): string {
  const escapedName = name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const pattern = new RegExp(`(^## ${escapedName}\\n)[\\s\\S]+?(^## |$)`, 'gm');

  if (!pattern.test(content)) {
    // Section doesn't exist - handled by caller
    return content;
  }

  // Reset lastIndex since we tested
  pattern.lastIndex = 0;
  // Use function replacement to prevent $ in newContent being interpreted as backreferences
  return content.replace(pattern, (match, p1, p2) => `${p1}\n${newContent}\n\n${p2}`);
}

// Append content to a section
export function appendToSection(content: string, name: string, addition: string): string {
  const sectionContent = extractSection(content, name);
  let newContent = sectionContent.trim();
  if (newContent !== '') {
    newContent += '\n';
  }
  newContent += addition;
  return replaceSection(content, name, newContent);
}

// Ensure a section exists, placing it before another section
export function ensureSection(content: string, name: string, before: string): string {
  const escapedName = name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const pattern = new RegExp(`^## ${escapedName}`, 'm');
  if (pattern.test(content)) {
    return content;
  }

  const escapedBefore = before.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const beforePattern = new RegExp(`(^## ${escapedBefore})`, 'm');

  if (beforePattern.test(content)) {
    return content.replace(beforePattern, `## ${name}\n\n$1`);
  }

  // If before section doesn't exist, append at end
  return content + `\n## ${name}\n\n`;
}

// Generate 4-character hash for an item
export function generateHash(text: string): string {
  const data = `${text}${Date.now()}${Math.random()}`;
  const hash = crypto.createHash('md5').update(data).digest('hex');
  return hash.substring(0, 4);
}

// Insert a timestamped entry to the Log section
export function insertLogEntry(content: string, entry: string): string {
  const now = new Date();
  const today = formatDate(now);
  const timestamp = formatTime(now);
  const bulletEntry = `- **${timestamp}** ${entry}`;
  const heading = `### ${today}`;

  // Check if today's heading exists
  const todayPattern = new RegExp(`^### ${today.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}`, 'm');
  if (todayPattern.test(content)) {
    // Insert after today's heading
    // Use function replacement to prevent $ in entry being interpreted as backreferences
    const pattern = new RegExp(`(^### ${today.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\n)`, 'm');
    return content.replace(pattern, (match, p1) => `${p1}\n${bulletEntry}\n`);
  }

  // Check if Log section exists
  const logPattern = /^## Log/m;
  if (logPattern.test(content)) {
    // Insert new heading after ## Log
    // Use function replacement to prevent $ in entry being interpreted as backreferences
    return content.replace(logPattern, () => `## Log\n\n${heading}\n\n${bulletEntry}`);
  }

  // No Log section - append one
  return content + `\n## Log\n\n${heading}\n\n${bulletEntry}\n`;
}

// Add a note to the Notes section with a hash comment
export function addNote(content: string, text: string): { content: string; hash: string } {
  // Ensure Notes section exists
  content = ensureSection(content, 'Notes', 'Todo');

  const hash = generateHash(text);
  const noteEntry = `- ${text}  <!-- ${hash} -->`;

  // Insert at top of Notes section
  // Use function replacement to prevent $ in text being interpreted as backreferences
  const pattern = /^## Notes\n/m;
  const newContent = content.replace(pattern, () => `## Notes\n\n${noteEntry}\n`);

  return { content: newContent, hash };
}

// Remove a line containing the specified hash comment from a section
export function removeByHash(content: string, section: string, hash: string): string {
  const lines = content.split('\n');
  let inSection = false;
  const hashPattern = `<!-- ${hash}`;
  let found = false;

  const result: string[] = [];
  for (const line of lines) {
    if (line.startsWith(`## ${section}`)) {
      inSection = true;
    } else if (line.startsWith('## ')) {
      inSection = false;
    }

    if (inSection && line.includes(hashPattern) && !found) {
      found = true;
      continue; // skip this line
    }
    result.push(line);
  }

  if (!found) {
    throw new Error(`no item with hash '${hash}' found`);
  }

  return result.join('\n');
}

// Edit item text by hash
export function editByHash(content: string, section: string, hash: string, newText: string): string {
  const lines = content.split('\n');
  let inSection = false;
  const hashPattern = `<!-- ${hash}`;
  let found = false;

  const hashCommentRe = /<!--\s*([a-f0-9]{4})\s*-->/;
  const result: string[] = [];

  for (const line of lines) {
    if (line.startsWith(`## ${section}`)) {
      inSection = true;
    } else if (line.startsWith('## ')) {
      inSection = false;
    }

    if (inSection && line.includes(hashPattern) && !found) {
      found = true;
      // Extract hash from line and rebuild
      const match = line.match(hashCommentRe);
      if (match) {
        result.push(`- ${newText}  <!-- ${match[1]} -->`);
        continue;
      }
    }
    result.push(line);
  }

  if (!found) {
    throw new Error(`no item with hash '${hash}' found`);
  }

  return result.join('\n');
}

// Add a todo item to the Todo section
export function addTodoItem(content: string, text: string): { content: string; hash: string } {
  const hash = generateHash(text);
  const todoEntry = `- [ ] ${text}  <!-- ${hash} -->`;

  // Insert at top of Todo section
  // Use function replacement to prevent $ in text being interpreted as backreferences
  const pattern = /^## Todo\n/m;
  const newContent = content.replace(pattern, () => `## Todo\n\n${todoEntry}\n`);

  return { content: newContent, hash };
}

// Set todo item's checked state by hash
export function setTodoChecked(content: string, hash: string, checked: boolean): string {
  const lines = content.split('\n');
  let inTodo = false;
  const hashPattern = `<!-- ${hash}`;
  let found = false;

  const result: string[] = [];
  for (let line of lines) {
    if (line.startsWith('## Todo')) {
      inTodo = true;
    } else if (line.startsWith('## ')) {
      inTodo = false;
    }

    if (inTodo && line.includes(hashPattern) && !found) {
      found = true;
      if (checked) {
        line = line.replace('- [ ]', '- [x]');
      } else {
        line = line.replace('- [x]', '- [ ]');
      }
    }
    result.push(line);
  }

  if (!found) {
    throw new Error(`no item with hash '${hash}' found`);
  }

  return result.join('\n');
}

// Count items matching a hash prefix in a section
export function countMatchingItems(content: string, section: string, hash: string): number {
  const lines = content.split('\n');
  let inSection = false;
  const hashPattern = `<!-- ${hash}`;
  let count = 0;

  for (const line of lines) {
    if (line.startsWith(`## ${section}`)) {
      inSection = true;
    } else if (line.startsWith('## ')) {
      inSection = false;
    }

    if (inSection && line.includes(hashPattern)) {
      count++;
    }
  }

  return count;
}
