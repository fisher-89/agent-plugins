import * as fs from 'fs';
import * as path from 'path';

import type z from 'zod/v4';

import { readEvalJson } from '../lib/eval-json';
import { type changeListOutputSchema, type changeListInputSchema } from '../schemas';
import { getProjectDir } from '../utils';

type ChangeListOptions = z.input<typeof changeListInputSchema>;

type ChangeListResult = z.output<typeof changeListOutputSchema>;

const KNOWN_ARTIFACTS = ['proposal.md', 'design.md', 'tasks.md', 'test-design.md', 'eval.json'];

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

/**
 * List all active (non-archived) changes under openspec/changes/.
 * Pure filesystem scan — no CLI dependency.
 */
export function runChangeList(options: ChangeListOptions): ChangeListResult {
  const projectRoot = options.project_root || getProjectDir();
  const changesDir = path.resolve(projectRoot, 'openspec', 'changes');

  if (!fs.existsSync(changesDir)) {
    return { project_root: projectRoot, changes: [], count: 0 };
  }

  const entries = fs.readdirSync(changesDir, { withFileTypes: true });
  const changes: ChangeListResult['changes'] = [];

  for (const entry of entries) {
    if (!entry.isDirectory() || entry.name === 'archive') continue;

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

    changes.push({
      name: entry.name,
      artifacts,
      tasks,
      latest_phase: latestPhase,
    });
  }

  changes.sort((a, b) => a.name.localeCompare(b.name));

  return { project_root: projectRoot, changes, count: changes.length };
}
