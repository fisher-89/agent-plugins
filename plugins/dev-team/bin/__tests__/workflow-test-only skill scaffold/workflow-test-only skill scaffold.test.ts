/**
 * 集成测试: workflow-test-only skill 静态结构与 scaffold 指令
 *
 * @see openspec/changes/workflow-test-only/test-design.md — AC-8, AC-15, AC-19
 */

import * as fs from 'node:fs';
import * as path from 'node:path';

import { describe, expect, it } from 'vite-plus/test';

/** vitest CWD 为 plugins/dev-team/bin/ */
const projectRoot = path.resolve(process.cwd(), '../../..');

const skillPath = path.resolve(projectRoot, 'plugins/dev-team/skills/workflow-test-only/SKILL.md');
const requirementSkillPath = path.resolve(
  projectRoot,
  'plugins/dev-team/skills/workflow-requirement/SKILL.md',
);

function readSkill(filePath: string): string {
  return fs.readFileSync(filePath, 'utf-8');
}

describe('workflow-test-only/SKILL.md — Step 1 workflow.json (AC-19)', () => {
  it('Step 1 明确写入 {"workflow_type": "test-only"}', () => {
    const content = readSkill(skillPath);
    expect(content).toMatch(/Write `openspec\/changes\/<name>\/workflow\.json`/);
    expect(content).toMatch(/\{"workflow_type": "test-only"\}/);
  });
});

describe('workflow-test-only/SKILL.md — 编排结构 (AC-8)', () => {
  it('Step 1 scaffold 后写入 workflow.json', () => {
    const content = readSkill(skillPath);
    expect(content).toMatch(/openspec new change/);
    expect(content).toMatch(/workflow\.json/);
  });

  it('Step 2 循环仅传 change 给 phase_next', () => {
    const content = readSkill(skillPath);
    const loopSection = content.split('### Step 2')[1]?.split('### Step 3')[0] ?? '';
    expect(loopSection).toMatch(/phase_next\(change=<name>\)/);
    expect(loopSection).not.toMatch(/workflow_type/);
  });

  it('六阶段 test-only 完成摘要 total phases: 6', () => {
    const content = readSkill(skillPath);
    expect(content).toMatch(/Total phases: 6/);
    expect(content).toMatch(/six test-only phases/);
  });
});

describe('workflow-test-only/SKILL.md — bug 报告流程 (AC-15)', () => {
  it('06/08 fail 且发现代码 bug 时写入 reports/code-bugs-found.md 并通知用户', () => {
    const content = readSkill(skillPath);
    expect(content).toMatch(/code-bugs-found\.md/);
    expect(content).toMatch(/code bugs found/i);
    expect(content).toMatch(/AskQuestion.*continue|terminate/is);
  });
});

describe('workflow-test-only vs workflow-requirement — Step 1 差异', () => {
  it('requirement skill Step 1 不写 test-only workflow.json', () => {
    const requirement = readSkill(requirementSkillPath);
    expect(requirement).not.toMatch(/\{"workflow_type": "test-only"\}/);
  });
});
