import * as fs from 'fs';
import * as path from 'path';

import type z from 'zod/v4';

import { readEvalJson } from '../lib/eval-json';
import { getPhaseTable } from '../lib/workflow';
import { type changeListOutputSchema } from '../schemas';
import { isPlainObject } from '../utils';
import { hasPhasePassed } from './phase-next';

type ChangeListResult = z.output<typeof changeListOutputSchema>;

const KNOWN_ARTIFACTS = ['proposal.md', 'design.md', 'tasks.md', 'test-design.md', 'eval.json'];
const WORKFLOW_JSON = 'workflow.json';

/**
 * Read the `workflow_type` from `openspec/changes/<change>/workflow.json`.
 * Returns `null` when the file is missing, unreadable, malformed, or lacks a
 * usable `workflow_type` string — callers treat `null` as "workflow not done".
 */
function readWorkflowType(changeDir: string): string | null {
  const filePath = path.join(changeDir, WORKFLOW_JSON);
  if (!fs.existsSync(filePath)) return null;
  let parsed: unknown;
  try {
    parsed = JSON.parse(fs.readFileSync(filePath, 'utf-8'));
  } catch {
    return null;
  }
  if (!isPlainObject(parsed) || typeof parsed.workflow_type !== 'string') return null;
  return parsed.workflow_type === '' ? null : parsed.workflow_type;
}

/**
 * Compute whether the change's workflow is complete: every phase in the
 * workflow's phase table has a non-stale pass/skipped entry in eval.json.
 *
 * Returns `false` (never throws) when workflow.json or eval.json is missing,
 * malformed, or the eval entry set is incomplete.
 */
function computeWorkflowDone(changeDir: string): boolean {
  const workflowType = readWorkflowType(changeDir);
  if (workflowType === null) return false;

  let entries: ReturnType<typeof readEvalJson>;
  try {
    entries = readEvalJson(changeDir);
  } catch {
    return false;
  }

  return getPhaseTable(workflowType).every((phase) => hasPhasePassed(entries, phase.id));
}

/**
 * Count total and completed tasks in a tasks.md file.
 * Tasks are lines matching `- [ ]` or `- [x]`.
 */
function countTasks(tasksPath: string): { total: number; done: number } {
  const content = fs.readFileSync(tasksPath, 'utf-8');
  let total = 0;
  let done = 0;
  for (const line of content.split('\n')) {
    if (/^\s*- \[/.test(line)) {
      total++;
      if (/^\s*- \[x\]/i.test(line)) {
        done++;
      }
    }
  }
  return { total, done };
}

function processChangeEntry(
  changesDir: string,
  entry: fs.Dirent,
): ChangeListResult['changes'][number] | null {
  if (!entry.isDirectory() || entry.name === 'archive') return null;

  const changeDir = path.join(changesDir, entry.name);

  const artifacts = KNOWN_ARTIFACTS.filter((a) => fs.existsSync(path.join(changeDir, a)));

  const tasksPath = path.join(changeDir, 'tasks.md');
  let tasks: { total: number; done: number } | null = null;
  if (fs.existsSync(tasksPath)) {
    try {
      tasks = countTasks(tasksPath);
    } catch {
      tasks = null;
    }
  }

  let latestPhase: ChangeListResult['changes'][number]['latest_phase'] = null;
  try {
    const evalEntries = readEvalJson(changeDir);
    if (evalEntries.length > 0) {
      const sorted = [...evalEntries].sort(
        (a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime(),
      );
      const latest = sorted[0];
      latestPhase = {
        phase: latest.phase,
        verdict: latest.verdict,
        ...(latest.stale ? { stale: true } : {}),
      };
    }
  } catch {
    // eval.json parse error — treat as absent
  }

  return {
    name: entry.name,
    artifacts,
    tasks,
    latest_phase: latestPhase,
    workflow_done: computeWorkflowDone(changeDir),
  };
}

/**
 * List all active (non-archived) changes under openspec/changes/.
 * Pure filesystem scan — no CLI dependency.
 */
export function runChangeList(projectRoot: string): ChangeListResult {
  const changesDir = path.resolve(projectRoot, 'openspec', 'changes');

  if (!fs.existsSync(changesDir)) {
    return { project_root: projectRoot, changes: [], count: 0 };
  }

  const entries = fs.readdirSync(changesDir, { withFileTypes: true });
  const changes: ChangeListResult['changes'] = [];

  for (const entry of entries) {
    const change = processChangeEntry(changesDir, entry);
    if (change) changes.push(change);
  }

  changes.sort((a, b) => a.name.localeCompare(b.name));

  return { project_root: projectRoot, changes, count: changes.length };
}
