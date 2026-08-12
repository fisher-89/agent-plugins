/**
 * 集成测试: phase-next run_id skill 文案静态契约
 *
 * 验证 workflow 与 phase skill 要求 turn 入口生成 run_id，
 * 且所有 __MCP:phase_next__ 调用包含 run_id=。
 *
 * @see openspec/changes/phase-next-session-round/test-design.md — AC-6
 */

import * as fs from 'node:fs';
import * as path from 'node:path';

import { describe, expect, it } from 'vite-plus/test';

const SKILLS_ROOT = path.resolve(__dirname, '../../../skills');

const WORKFLOW_SKILLS = ['workflow-requirement', 'workflow-test-only'] as const;

const PHASE_SKILLS = [
  'phase-proposal',
  'phase-dev-design',
  'phase-test-design',
  'phase-implement',
  'phase-test-gen',
  'phase-test-execution',
  'phase-code-review',
  'phase-acceptance',
] as const;

function readSkill(skillDir: string): string {
  const skillPath = path.join(SKILLS_ROOT, skillDir, 'SKILL.md');
  if (!fs.existsSync(skillPath)) {
    throw new Error(`缺少 skill 文件: ${skillPath}`);
  }
  return fs.readFileSync(skillPath, 'utf-8');
}

function extractPhaseNextCalls(content: string): string[] {
  return content.match(/__MCP:phase_next__[^`\n]*/g) ?? [];
}

function assertAllPhaseNextCallsIncludeRunId(content: string, skillDir: string): void {
  const calls = extractPhaseNextCalls(content);
  expect(calls.length).toBeGreaterThan(0);
  for (const call of calls) {
    expect(call, `${skillDir} 中 phase_next 调用缺少 run_id=: ${call}`).toMatch(/run_id=/);
  }
}

describe('workflow skill — turn 入口 run_id (AC-6)', () => {
  for (const skillDir of WORKFLOW_SKILLS) {
    it(`${skillDir} 要求 turn 入口生成 run_id 且每个 phase_next 含 run_id=`, () => {
      const content = readSkill(skillDir);
      expect(content).toMatch(/generate a new non-empty opaque `run_id`/i);
      assertAllPhaseNextCallsIncludeRunId(content, skillDir);
    });
  }

  it('workflow-requirement 与 workflow-test-only 均要求 ask-user / 下一用户消息新生成 run_id', () => {
    for (const skillDir of WORKFLOW_SKILLS) {
      const content = readSkill(skillDir);
      expect(content).toMatch(
        /Each new user message \(including replies after `ask-user`\) MUST generate a fresh `run_id`/,
      );
    }
  });

  it('两份 workflow skill 进度行仍为 [Round {gate.round}/20] 且注明 session 语义', () => {
    for (const skillDir of WORKFLOW_SKILLS) {
      const content = readSkill(skillDir);
      expect(content).toContain('[Round {gate.round}/20]');
      expect(content).toMatch(/session-scoped|session round/i);
    }
  });

  it('不得残留仅 phase_next(change=...) 且无 run_id 的调用形态', () => {
    for (const skillDir of WORKFLOW_SKILLS) {
      const content = readSkill(skillDir);
      const badCalls = extractPhaseNextCalls(content).filter(
        (call) => /change=/.test(call) && !/run_id=/.test(call),
      );
      expect(badCalls).toEqual([]);
    }
  });
});

describe('phase skill — 独立传入 run_id (AC-6)', () => {
  it('枚举恰好 8 个目标 phase skill 目录且文件存在', () => {
    expect(PHASE_SKILLS).toHaveLength(8);
    for (const skillDir of PHASE_SKILLS) {
      expect(fs.existsSync(path.join(SKILLS_ROOT, skillDir, 'SKILL.md'))).toBe(true);
    }
  });

  for (const skillDir of PHASE_SKILLS) {
    it(`${skillDir} 要求 turn 入口生成 run_id`, () => {
      const content = readSkill(skillDir);
      expect(content).toMatch(/generate a new non-empty opaque `run_id`/i);
    });

    it(`${skillDir} 中全部 __MCP:phase_next__ 调用含 run_id=`, () => {
      const content = readSkill(skillDir);
      assertAllPhaseNextCallsIncludeRunId(content, skillDir);
    });

    it(`${skillDir} backtrack recall 与 Phase Check 使用同一 turn run_id`, () => {
      const content = readSkill(skillDir);
      expect(content).toMatch(/Pass the same `run_id` to every `phase_next` call in this turn/i);
      if (/recall `__MCP:phase_next__/.test(content)) {
        expect(content).toMatch(/recall `__MCP:phase_next__[^`]*run_id=/);
      }
    });
  }

  it('任一 phase skill 不得残留无 run_id= 的 phase_next 调用', () => {
    for (const skillDir of PHASE_SKILLS) {
      const content = readSkill(skillDir);
      const badCalls = extractPhaseNextCalls(content).filter((call) => !/run_id=/.test(call));
      expect(badCalls, `${skillDir} 存在无 run_id 的调用`).toEqual([]);
    }
  });
});
