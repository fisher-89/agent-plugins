import * as fs from 'fs';
import * as path from 'path';

import { isPlainObject } from '../utils';
import { getChangeDir } from './change';
import { getProjectDir } from './project-root';

const WORKFLOW_JSON = 'workflow.json';
const DEFAULT_WORKFLOW_TYPE = 'requirement';

/**
 * Read and parse `openspec/changes/<change>/workflow.json`.
 * Returns `{}` when the file does not exist.
 * Throws a readable error when JSON is invalid.
 */
function readWorkflowConfig(change: string): Record<string, unknown> {
  const filePath = path.join(getChangeDir(change, getProjectDir()), WORKFLOW_JSON);
  if (!fs.existsSync(filePath)) {
    return {};
  }
  const raw = fs.readFileSync(filePath, 'utf-8');
  try {
    const parsed: unknown = JSON.parse(raw);
    if (!isPlainObject(parsed)) {
      throw new Error(`workflow.json 根元素必须是对象，但实际类型为 ${typeof parsed}`);
    }
    return parsed;
  } catch (e: unknown) {
    if (e instanceof SyntaxError) {
      throw new Error(`workflow.json 解析失败: ${e.message}`);
    }
    throw e;
  }
}

/**
 * Return `workflow_type` from change `workflow.json`.
 * Defaults to `"requirement"` when the file or field is absent.
 */
export function getWorkflowType(change: string): string {
  const config = readWorkflowConfig(change);
  const value = config.workflow_type;
  if (typeof value === 'string' && value !== '') {
    return value;
  }
  return DEFAULT_WORKFLOW_TYPE;
}
