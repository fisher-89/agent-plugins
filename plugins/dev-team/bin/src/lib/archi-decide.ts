/**
 * archi-decide.ts — Architecture Decision Record (ADR) create / list / update.
 * Ported from Python archi-decide.py to MCP-backed TypeScript lib.
 */

import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

import type {
  AdrStatus,
  ArchiDecideAlternative,
  ArchiDecideCreateInput,
  ArchiDecideCreateResult,
  ArchiDecideListResult,
  ArchiDecideUpdateInput,
  ArchiDecideUpdateResult,
} from '../schemas/archi-decide.schema';

const DECISIONS_DIR = path.join('openspec', 'architecture', 'decisions');
const VALID_STATUSES: readonly AdrStatus[] = ['proposed', 'accepted', 'deprecated', 'superseded'];
const REQUIRED_TEMPLATE_HEADINGS = ['## 背景', '## 决策', '## 后果', '## 备选方案', '## 影响范围'];

function slugify(title: string): string {
  let slug = title.toLowerCase().trim();
  slug = slug.replace(/[^\w\s-]/g, '');
  slug = slug.replace(/[\s_]+/g, '-');
  slug = slug.replace(/-+/g, '-');
  return slug.replace(/^-+|-+$/g, '');
}

function getModuleStartDir(): string {
  if (typeof __dirname === 'string') {
    return __dirname;
  }
  return path.dirname(fileURLToPath(import.meta.url));
}

function resolveAdrTemplatePath(): string | null {
  let dir = getModuleStartDir();
  while (true) {
    const candidate = path.join(dir, 'templates', 'adr.md');
    if (fs.existsSync(candidate)) {
      return candidate;
    }
    const parent = path.dirname(dir);
    if (parent === dir) {
      return null;
    }
    dir = parent;
  }
}

function formatProsCons(value: string | string[] | undefined, label: '优点' | '缺点'): string {
  if (value === undefined) {
    return '';
  }
  if (Array.isArray(value)) {
    return value.map((item) => `- **${label}**: ${item}`).join('\n');
  }
  return `- **${label}**: ${value}`;
}

function renderAlternatives(alternatives: ArchiDecideAlternative[] | undefined): string {
  if (!alternatives?.length) {
    return '\n(无备选方案记录)\n';
  }

  let text = '';
  for (let i = 0; i < alternatives.length; i += 1) {
    const alt = alternatives[i];
    if (!alt) {
      continue;
    }
    text += `\n### 方案 ${i + 1}：${alt.name}\n`;
    text += `- **描述**: ${alt.description ?? alt.name}\n`;
    const pros = formatProsCons(alt.pros, '优点');
    if (pros) {
      text += `${pros}\n`;
    }
    const cons = formatProsCons(alt.cons, '缺点');
    if (cons) {
      text += `${cons}\n`;
    }
  }
  return text;
}

function renderScope(scope: string[] | undefined): string {
  const items = scope?.map((item) => item.trim()).filter(Boolean) ?? [];
  if (items.length === 0) {
    return '- (none)';
  }
  return items.map((item) => `- ${item}`).join('\n');
}

function replaceSectionBody(
  content: string,
  startHeading: string,
  endHeading: string,
  body: string,
): string {
  const startIdx = content.indexOf(startHeading);
  if (startIdx < 0) {
    return content;
  }
  const bodyStart = startIdx + startHeading.length;
  const endIdx = content.indexOf(endHeading, bodyStart);
  if (endIdx < 0) {
    return content;
  }
  const prefix = content.slice(0, bodyStart);
  const suffix = content.slice(endIdx);
  return `${prefix}\n\n${body}\n\n${suffix}`;
}

function replaceTrailingSectionBody(content: string, heading: string, body: string): string {
  const startIdx = content.indexOf(heading);
  if (startIdx < 0) {
    return content;
  }
  const bodyStart = startIdx + heading.length;
  return `${content.slice(0, bodyStart)}\n\n${body}\n`;
}

function renderAdrFromTemplate(
  template: string,
  data: {
    title: string;
    date: string;
    status: AdrStatus;
    background: string;
    decision: string;
    consequences: string;
    alternatives: ArchiDecideAlternative[] | undefined;
    scope: string[] | undefined;
  },
): string {
  let content = template;

  content = content.replace('# ADR: <title>', `# ADR: ${data.title}`);
  content = content.replace('YYYY-MM-DD', data.date);
  content = content.replace(
    '- **状态**: proposed | accepted | deprecated | superseded',
    `- **状态**: ${data.status}`,
  );
  content = content.replace('<描述导致此决策的技术问题或业务需求>', data.background);
  content = content.replace('<宣布并解释所做的决定>', data.decision);
  content = content.replace('- <列出选择带来的好处>', '- (见下方描述)');
  content = content.replace('- <列出选择带来的代价、限制或风险>', '- (见下方描述)');

  if (data.consequences) {
    const marker = '- (见下方描述)';
    const lastIdx = content.lastIndexOf(marker);
    if (lastIdx >= 0) {
      const insertAt = lastIdx + marker.length;
      content = `${content.slice(0, insertAt)}\n\n${data.consequences}${content.slice(insertAt)}`;
    }
  }

  content = replaceSectionBody(
    content,
    '## 备选方案',
    '## 影响范围',
    renderAlternatives(data.alternatives).trim(),
  );

  const scopeBody = `<!-- 引用受此决策影响的模型元素 FQN -->\n\n${renderScope(data.scope)}`;
  content = replaceTrailingSectionBody(content, '## 影响范围', scopeBody);

  return content.endsWith('\n') ? content : `${content}\n`;
}

function isValidStatus(status: string): status is AdrStatus {
  return (VALID_STATUSES as readonly string[]).includes(status);
}

function invalidStatusError(status: string): { success: false; error: string } {
  return {
    success: false,
    error: `Invalid status '${status}'. Must be one of: ${VALID_STATUSES.join(', ')}`,
  };
}

function localDateString(): string {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, '0');
  const day = String(now.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

function loadAdrTemplate(): { template: string } | { error: string } {
  const templatePath = resolveAdrTemplatePath();
  if (!templatePath) {
    return { error: 'ADR template not found: templates/adr.md' };
  }

  let template: string;
  try {
    template = fs.readFileSync(templatePath, 'utf-8');
  } catch {
    return { error: `Failed to read ADR template: ${templatePath}` };
  }

  for (const heading of REQUIRED_TEMPLATE_HEADINGS) {
    if (!template.includes(heading)) {
      return { error: `ADR template missing required section: ${heading}` };
    }
  }

  return { template };
}

function writeAdrFile(
  decisionsDir: string,
  filepath: string,
  content: string,
): { success: true } | { success: false; error: string } {
  try {
    fs.mkdirSync(decisionsDir, { recursive: true });
    fs.writeFileSync(filepath, content, 'utf-8');
    return { success: true };
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return { success: false, error: `Failed to write ADR: ${message}` };
  }
}

export function createAdr(
  projectRoot: string,
  input: ArchiDecideCreateInput,
): ArchiDecideCreateResult {
  const statusValue = input.status ?? 'proposed';
  if (!isValidStatus(statusValue)) {
    return invalidStatusError(statusValue);
  }

  const loaded = loadAdrTemplate();
  if ('error' in loaded) {
    return { success: false, error: loaded.error };
  }

  const dateStr = localDateString();
  const filename = `${dateStr}-${slugify(input.title)}.md`;
  const decisionsDir = path.join(projectRoot, DECISIONS_DIR);
  const filepath = path.join(decisionsDir, filename);

  if (fs.existsSync(filepath)) {
    return { success: false, error: `ADR already exists: ${filename}` };
  }

  const content = renderAdrFromTemplate(loaded.template, {
    title: input.title,
    date: dateStr,
    status: statusValue,
    background: input.background,
    decision: input.decision,
    consequences: input.consequences ?? '',
    alternatives: input.alternatives,
    scope: input.scope,
  });

  const written = writeAdrFile(decisionsDir, filepath, content);
  if (!written.success) {
    return written;
  }

  return { success: true, path: filepath, filename };
}

function parseAdrFile(filepath: string, filename: string): ArchiDecideListResult['adrs'][number] {
  const content = fs.readFileSync(filepath, 'utf-8');

  const titleMatch = content.match(/^# ADR:\s*(.+)$/m);
  const title = titleMatch?.[1]?.trim() ?? filename;

  const statusMatch = content.match(/\*\*状态\*\*:\s*(\w+)/);
  const status = statusMatch?.[1]?.trim() ?? 'unknown';

  const dateMatch = content.match(/\*\*日期\*\*:\s*(\S+)/);
  const date = dateMatch?.[1]?.trim() ?? '';

  const scopeSection = content.match(/## 影响范围\s*\n([\s\S]*?)(?=\n##|$)/);
  const scope: string[] = [];
  const scopeBody = scopeSection?.[1];
  if (scopeBody) {
    for (const line of scopeBody.trim().split('\n')) {
      const trimmed = line.trim();
      if (trimmed.startsWith('-')) {
        scope.push(trimmed.replace(/^-\s*/, '').trim());
      }
    }
  }

  return { filename, title, status, date, scope, path: filepath };
}

export function listAdrs(projectRoot: string, statusFilter?: AdrStatus): ArchiDecideListResult {
  const decisionsDir = path.join(projectRoot, DECISIONS_DIR);

  if (!fs.existsSync(decisionsDir) || !fs.statSync(decisionsDir).isDirectory()) {
    return { adrs: [], count: 0 };
  }

  const adrs: ArchiDecideListResult['adrs'] = [];

  let filenames: string[];
  try {
    filenames = fs.readdirSync(decisionsDir).filter((name) => name.endsWith('.md'));
  } catch {
    return { adrs: [], count: 0 };
  }

  for (const filename of filenames) {
    const filepath = path.join(decisionsDir, filename);
    try {
      if (!fs.statSync(filepath).isFile()) {
        continue;
      }
      const entry = parseAdrFile(filepath, filename);
      if (statusFilter && entry.status !== statusFilter) {
        continue;
      }
      adrs.push(entry);
    } catch {
      continue;
    }
  }

  adrs.sort((a, b) => {
    if (a.date !== b.date) {
      return b.date.localeCompare(a.date);
    }
    return b.filename.localeCompare(a.filename);
  });

  return { adrs, count: adrs.length };
}

function applyStatusUpdate(
  content: string,
  status: AdrStatus,
  supersededBy: string | undefined,
): { content: string; oldStatus: string } | { error: string } {
  const oldStatusMatch = content.match(/\*\*状态\*\*:\s*(\w+)/);
  const oldStatus = oldStatusMatch?.[1]?.trim() ?? 'unknown';

  const statusLineMatch = content.match(/(\*\*状态\*\*:\s*)\w+/);
  if (!statusLineMatch || statusLineMatch.index === undefined) {
    return { error: 'ADR file missing **状态** field' };
  }

  let updated =
    content.slice(0, statusLineMatch.index) +
    `**状态**: ${status}` +
    content.slice(statusLineMatch.index + statusLineMatch[0].length);

  if (status === 'superseded' && supersededBy && !updated.includes('**取代者**')) {
    const statusEnd = updated.indexOf('\n', updated.indexOf(`**状态**: ${status}`));
    const refLine = `\n- **取代者**: ${supersededBy}`;
    if (statusEnd > 0) {
      updated = updated.slice(0, statusEnd) + refLine + updated.slice(statusEnd);
    }
  }

  return { content: updated, oldStatus };
}

function resolveAdrFilePath(
  projectRoot: string,
  file: string,
): { filepath: string } | { error: string } {
  const decisionsDir = path.resolve(projectRoot, DECISIONS_DIR);
  const filepath = path.resolve(decisionsDir, file);
  const relative = path.relative(decisionsDir, filepath);
  if (relative.startsWith('..') || path.isAbsolute(relative)) {
    return { error: `Path '${file}' is outside decisions directory` };
  }
  return { filepath };
}

export function updateAdr(
  projectRoot: string,
  input: ArchiDecideUpdateInput,
): ArchiDecideUpdateResult {
  const statusValue = input.status;
  if (!isValidStatus(statusValue)) {
    return invalidStatusError(statusValue);
  }

  if (statusValue === 'superseded' && !input.superseded_by) {
    return {
      success: false,
      error: "Status 'superseded' requires superseded_by reference",
    };
  }

  const resolved = resolveAdrFilePath(projectRoot, input.file);
  if ('error' in resolved) {
    return { success: false, error: resolved.error };
  }
  const filepath = resolved.filepath;
  if (!fs.existsSync(filepath) || !fs.statSync(filepath).isFile()) {
    return { success: false, error: `ADR not found: ${input.file}` };
  }

  let content: string;
  try {
    content = fs.readFileSync(filepath, 'utf-8');
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return { success: false, error: `Failed to read ADR: ${message}` };
  }

  const applied = applyStatusUpdate(content, statusValue, input.superseded_by);
  if ('error' in applied) {
    return { success: false, error: applied.error };
  }

  const written = writeAdrFile(path.dirname(filepath), filepath, applied.content);
  if (!written.success) {
    return written;
  }

  return {
    success: true,
    path: filepath,
    old_status: applied.oldStatus,
    new_status: statusValue,
  };
}
