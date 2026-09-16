/**
 * 集成测试: Skill / agent 文案 → phase_log / workflow.json
 *
 * Skill / agent 是 agent 运行时的操作说明书，错误文件名会诱导 Write 被 hook deny，
 * 或在缺文件时依赖已删除的缺省类型继续门禁。本关系用只读 `fs.readFileSync` 做静态
 * 文案契约，不启动 agent。
 *
 * 涉及模块:
 * - `plugins/dev-team/skills/phase-proposal/SKILL.md`
 * - `plugins/dev-team/skills/workflow-requirement/SKILL.md`
 * - `plugins/dev-team/skills/workflow-test-only/SKILL.md`
 * - `plugins/dev-team/skills/openspec-archive-change/SKILL.md`
 * - `plugins/dev-team/agents/*-evaluator.md`、`test-execution-executor.md`、
 *   `test-design-planner.md`
 *
 * 关联 AC: AC-11
 */

import * as fs from 'fs';
import * as path from 'path';
import { fileURLToPath } from 'url';

import { describe, expect, it } from 'vite-plus/test';

const PLUGIN_ROOT = path.resolve(fileURLToPath(new URL('.', import.meta.url)), '..', '..', '..');
const SKILLS_DIR = path.join(PLUGIN_ROOT, 'skills');
const AGENTS_DIR = path.join(PLUGIN_ROOT, 'agents');

const PHASE_PROPOSAL = path.join(SKILLS_DIR, 'phase-proposal', 'SKILL.md');
const WORKFLOW_REQUIREMENT = path.join(SKILLS_DIR, 'workflow-requirement', 'SKILL.md');
const WORKFLOW_TEST_ONLY = path.join(SKILLS_DIR, 'workflow-test-only', 'SKILL.md');
const ARCHIVE_CHANGE = path.join(SKILLS_DIR, 'openspec-archive-change', 'SKILL.md');
const TEST_DESIGN_PLANNER = path.join(AGENTS_DIR, 'test-design-planner.md');

const EVALUATOR_FILES = [
  'acceptance-evaluator.md',
  'code-analyze-evaluator.md',
  'code-review-evaluator.md',
  'dev-design-evaluator.md',
  'implementation-evaluator.md',
  'proposal-evaluator.md',
  'test-design-evaluator.md',
  'test-execution-evaluator.md',
  'test-gen-evaluator.md',
].map((name) => path.join(AGENTS_DIR, name));

const EXECUTOR_FILE = path.join(AGENTS_DIR, 'test-execution-executor.md');

function read(filePath: string): string {
  return fs.readFileSync(filePath, 'utf-8');
}

function readLines(filePath: string): string[] {
  return read(filePath).split('\n');
}

/** 否定语气标记：这些行是「禁止直接写」，不是把该文件当作写入目标。 */
const NEGATION = /(do not|don't|must not|never|不要|不得|禁止|instead of)/i;

/**
 * 返回把 `eval.json` 当作写入目标的行（排除否定语气与「不是 artifact」的说明）。
 */
function evalJsonWriteTargets(filePath: string): string[] {
  return readLines(filePath).filter(
    (line) =>
      line.includes('eval.json') &&
      !NEGATION.test(line) &&
      !/not an artifact/i.test(line) &&
      !/^\s*-?\s*`?eval\.json`?\s*$/i.test(line.trim()),
  );
}

// ===========================================================================
// 场景: 文案指向 phase_log 且禁止手写 JSON
// ===========================================================================

describe('phase-proposal/SKILL.md — 禁止手写 workflow.json 并给出缺文件指引（AC-11）', () => {
  it('不存在要求 Write `workflow.json` 的步骤', () => {
    const offenders = readLines(PHASE_PROPOSAL).filter(
      (line) => /\bWrite\b/.test(line) && line.includes('workflow.json') && !NEGATION.test(line),
    );

    expect(offenders).toEqual([]);
  });

  it('缺文件分支要求停止（STOP），且 MUST NOT 用 Write/Edit 创建该文件', () => {
    const content = read(PHASE_PROPOSAL);

    expect(content).toMatch(/\*\*STOP\*\*/);
    expect(content).toMatch(/MUST NOT.*Write\/Edit to create the file/);
    expect(content).toMatch(/a missing file is an error, not a default/);
  });

  it('缺文件分支含「手写 / change_create」二选一指引', () => {
    const content = read(PHASE_PROPOSAL);

    // (a) 用户自行手写，且给出 workflowFileSchema 形状
    expect(content).toMatch(/\(a\).*writes `openspec\/changes\/<name>\/workflow\.json` themselves/);
    expect(content).toContain('workflowFileSchema');
    expect(content).toContain('"workflow_type": "<requirement|bug-fix|refactor|test-only>"');
    // (b) 或经 change_create 重建（并说明已存在目录会被拒绝）
    expect(content).toMatch(/\(b\).*Re-create the change/);
    expect(content).toMatch(/change_create.*rejects an already existing directory/);
  });

  it('缺文件分支不再是无条件调用 change_create（对已存在目录必然失败，语义已废弃）', () => {
    const lines = readLines(PHASE_PROPOSAL);
    const start = lines.findIndex((line) => line.startsWith('- **Does not exist**'));
    expect(start).toBeGreaterThanOrEqual(0);

    // 缺文件分支的正文：直到下一个同级标题或空行分隔的下一个要点
    const branch = lines.slice(start, start + 4).join('\n');

    expect(branch).toContain('**STOP**');
    // 停止后不得无条件调用 change_create（只允许出现在 (b) 重建选项里）
    expect(branch).not.toContain('__MCP:change_create__');

    // 整个分支（含两个选项）里 change_create 只作为「可丢弃目录」的重建方案出现
    const fullBranch = lines.slice(start, start + 20).join('\n');
    expect(fullBranch).toMatch(/\(b\).*change_create/);
  });
});

describe('evaluator / executor 文案 — 经 phase_log 写 workflow.json（AC-11）', () => {
  it('各 evaluator 均提及 phase_log 与 workflow.json，且不以「Append to eval.json」为步骤标题', () => {
    for (const filePath of EVALUATOR_FILES) {
      const content = read(filePath);

      expect(content, filePath).toContain('phase_log');
      expect(content, filePath).toContain('workflow.json');
      expect(content, filePath).not.toMatch(/Append(ing)? to `?eval\.json`?/i);
    }
  });

  it('evaluator 的 phase_log 步骤说明写入 workflow.json 的 eval 字段', () => {
    for (const filePath of EVALUATOR_FILES) {
      expect(read(filePath), filePath).toMatch(/phase_log.*workflow\.json/);
    }
  });

  it('test-execution-executor 说明评估落盘由 evaluator 经 phase_log 写入 workflow.json', () => {
    const content = read(EXECUTOR_FILE);

    expect(content).toContain('phase_log');
    expect(content).toMatch(/phase_log.*workflow\.json/);
    expect(content).toMatch(/do NOT directly write `eval\.json` or `workflow\.json`/);
  });

  it('没有任何 evaluator / executor 把 eval.json 当作写入目标', () => {
    for (const filePath of [...EVALUATOR_FILES, EXECUTOR_FILE]) {
      expect(evalJsonWriteTargets(filePath), filePath).toEqual([]);
    }
  });
});

describe('workflow-*.SKILL.md — workflow.json 由 change_create 建立（AC-11）', () => {
  it.each([
    ['workflow-requirement', WORKFLOW_REQUIREMENT],
    ['workflow-test-only', WORKFLOW_TEST_ONLY],
  ])(
    '%s 说明 change_create 是 workflow.json 的唯一创建者，且评估经 phase_log 写入 eval 字段',
    (_name, filePath) => {
      const content = read(filePath);

      expect(content).toContain('__MCP:change_create__');
      expect(content).toMatch(/`change_create` is the ONLY creator of `workflow\.json`/);
      expect(content).toContain('workflow.json.eval');
      expect(content).toMatch(/never by writing the file directly/);
      expect(evalJsonWriteTargets(filePath)).toEqual([]);
    },
  );

  it('workflow-test-only 说明 evaluator 经 phase_log 写入 workflow.json 的 eval 字段', () => {
    expect(read(WORKFLOW_TEST_ONLY)).toMatch(
      /__MCP:phase_log__.*写入 `workflow\.json`（`eval` 字段）/,
    );
  });
});

describe('openspec-archive-change/SKILL.md — 完成性以 workflow_done 为准（AC-11）', () => {
  it('artifacts 示例不含 eval.json，且说明评估历史不是 artifact', () => {
    const content = read(ARCHIVE_CHANGE);

    expect(content).toContain('evaluation history is not an artifact');
    expect(content).toContain('workflow.json eval');
    expect(content).toMatch(/`workflow_done`/);
  });

  it('提示 workflow_done 为 false 可能源于 workflow.json 缺失 / 格式非法，并指引 change_create', () => {
    const content = read(ARCHIVE_CHANGE);

    expect(content).toMatch(/lacks `workflow\.json`, or the file is malformed/);
    expect(content).toMatch(/`workflow_done` is\s*\n?\s*also `false`/);
    expect(content).toMatch(/__MCP:change_create__/);
  });

  it('归档完成性检查不把 eval.json 当作 artifact 或写入目标', () => {
    expect(evalJsonWriteTargets(ARCHIVE_CHANGE)).toEqual([]);
  });
});

describe('test-design-planner.md — 关系标题示例指向 workflow.json（AC-11）', () => {
  it('关系标题示例为 `CLI参数 → workflow.json持久化`，不再出现 `eval.json持久化`', () => {
    const content = read(TEST_DESIGN_PLANNER);

    expect(content).toContain('CLI参数 → workflow.json持久化');
    expect(content).not.toContain('eval.json持久化');
    expect(evalJsonWriteTargets(TEST_DESIGN_PLANNER)).toEqual([]);
  });
});
