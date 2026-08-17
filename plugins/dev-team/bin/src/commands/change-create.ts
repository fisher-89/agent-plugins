import * as fs from 'fs';
import * as path from 'path';

import { getChangeDir } from '../lib/change';
import { kebabCasePattern } from '../schemas';

const MAX_NAME_LENGTH = 128;

/**
 * Create a new change directory with a default workflow.json metadata file.
 * Replaces `openspec new change` from the bundled openspec CLI.
 *
 * - Validates `name` is kebab-case (`^[a-z0-9][a-z0-9-]*$`, max 128 chars).
 * - Rejects the change if `openspec/changes/<name>/` already exists.
 * - Creates the directory and writes `workflow.json`.
 * - Does NOT write `.openspec.yaml`.
 *
 * Returns `{ name, path }` where `path` is the created change directory.
 */
export function runChangeCreate(
  name: string,
  projectRoot: string,
  workflowType: string,
): {
  name: string;
  path: string;
} {
  if (!kebabCasePattern.test(name)) {
    throw new Error(
      `name 必须为 kebab-case（小写字母/数字，可用 \`-\` 连接，最长 128 字符），收到: ${JSON.stringify(name)}`,
    );
  }
  if (name.length > MAX_NAME_LENGTH) {
    throw new Error(`name 长度超过 ${MAX_NAME_LENGTH} 字符限制（当前 ${name.length} 字符）`);
  }

  const changeDir = getChangeDir(name, projectRoot);
  if (fs.existsSync(changeDir)) {
    throw new Error(`change "${name}" 已存在: ${changeDir}`);
  }

  fs.mkdirSync(changeDir, { recursive: true });
  const created = new Date().toISOString().slice(0, 10);
  const workflow = { workflow_type: workflowType, created };
  fs.writeFileSync(path.join(changeDir, 'workflow.json'), `${JSON.stringify(workflow)}\n`, 'utf-8');

  return { name, path: changeDir };
}
