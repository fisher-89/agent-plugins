import * as fs from 'fs';
import * as path from 'path';

/**
 * Extract a description from a spec.md file: the first non-empty paragraph
 * following the `## <name>` heading (the heading itself is the last `## ...`
 * heading directly above the first paragraph).
 *
 * Returns an empty string when no suitable paragraph is found.
 */
function extractDescription(content: string, name: string): string {
  const lines = content.split(/\r?\n/);
  let inNameSection = false;
  for (const line of lines) {
    if (/^##\s/.test(line)) {
      const heading = line.replace(/^##\s+/, '').trim();
      inNameSection = heading === name;
      continue;
    }
    if (!inNameSection) continue;
    const trimmed = line.trim();
    if (trimmed === '') continue;
    if (/^#{1,6}\s/.test(trimmed) || trimmed.startsWith('```')) continue;
    return trimmed;
  }
  return '';
}

/**
 * Scan `openspec/specs/<name>/spec.md` (one per capability) and return a flat capability list.
 * Replaces `openspec spec list --json` from the retired bundled openspec CLI.
 *
 * - Only the `openspec/specs` top-level directory (single level) is scanned.
 * - The description is the first non-empty paragraph under the `<name>` heading.
 * - Unreadable or malformed spec files are silently skipped.
 */
export function runSpecList(projectRoot: string): {
  project_root: string;
  specs: { name: string; path: string; description: string }[];
} {
  const specsRoot = path.resolve(projectRoot, 'openspec', 'specs');
  const specs: { name: string; path: string; description: string }[] = [];

  if (!fs.existsSync(specsRoot)) {
    return { project_root: projectRoot, specs };
  }

  let entries: fs.Dirent[];
  try {
    entries = fs.readdirSync(specsRoot, { withFileTypes: true });
  } catch {
    return { project_root: projectRoot, specs };
  }

  for (const entry of entries) {
    if (!entry.isDirectory()) continue;
    const specPath = path.join(specsRoot, entry.name, 'spec.md');
    if (!fs.existsSync(specPath)) continue;
    let content: string;
    try {
      content = fs.readFileSync(specPath, 'utf-8');
    } catch {
      continue;
    }
    specs.push({
      name: entry.name,
      path: specPath,
      description: extractDescription(content, entry.name),
    });
  }

  specs.sort((a, b) => a.name.localeCompare(b.name));
  return { project_root: projectRoot, specs };
}
