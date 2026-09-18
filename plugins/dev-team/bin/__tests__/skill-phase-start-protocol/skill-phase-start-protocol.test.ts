/**
 * 集成测试（文本断言）: skills 下 8 个 phase 技能与 2 个 workflow 编排技能的
 * SKILL.md — phase_next → phase_start 调用协议插入点
 *
 * 覆盖范围（openspec/changes/phase-lifecycle-file-log/test-design.md）:
 * - AC-11: 8 个 `skills/phase-<id>/SKILL.md` 在 gate `phase_next` 返回本 phase 后、
 *   executor 执行前含 `__MCP:phase_start__` 调用协议；retry 重跑与 backtrack recall
 *   重进执行前重新调用；Verdict `phase_next` 不触发
 * - `skills/workflow-requirement/SKILL.md`、`skills/workflow-test-only/SKILL.md`:
 *   LOOP 内 gate 通过后、executor 执行前含 phase_start（每轮迭代一次；done/error 不调用）
 * - 文本缺失（技能漏改）→ 断言失败并列出缺失文件名
 *
 * Mock 策略: 无——真实 fs 读取仓库内 SKILL.md 源文本（参照 eval-store-skill-copy
 * 的 readFileSync + 断言模式）。
 */

import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vite-plus/test';

const PLUGIN_ROOT = path.resolve(fileURLToPath(new URL('.', import.meta.url)), '..', '..', '..');
const SKILLS_DIR = path.join(PLUGIN_ROOT, 'skills');

const PHASE_SKILLS = [
  'phase-proposal',
  'phase-dev-design',
  'phase-test-design',
  'phase-implement',
  'phase-test-gen',
  'phase-test-execution',
  'phase-code-review',
  'phase-acceptance',
];

const WORKFLOW_SKILLS = ['workflow-requirement', 'workflow-test-only'];

function readSkill(skill: string): string {
  return fs.readFileSync(path.join(SKILLS_DIR, skill, 'SKILL.md'), 'utf-8');
}

/**
 * phase 技能协议断言：返回缺失项列表（空数组 = 全部通过）。
 * - gate `phase_next` 引用先于 `__MCP:phase_start__` 调用（序列 phase_next → phase_start）
 * - phase_start 调用位于 `### Run Executor` 之后（gate 返回本 phase 后、executor 执行前）
 * - retry / backtrack recall 重跑语境的「重新调用」文本存在
 * - Verdict 段落不新增 phase_start 调用
 */
function checkPhaseSkillProtocol(text: string): string[] {
  const missing: string[] = [];

  const nextIdx = text.indexOf('__MCP:phase_next__');
  const startIdx = text.indexOf('__MCP:phase_start__');
  if (nextIdx === -1) missing.push('缺少 __MCP:phase_next__ 引用（gate 序列起点）');
  if (startIdx === -1) {
    missing.push('缺少 __MCP:phase_start__ 调用协议');
    return missing;
  }
  if (nextIdx !== -1 && nextIdx > startIdx) {
    missing.push('phase_start 出现在 phase_next 之前（序列应为 phase_next → phase_start）');
  }

  // phase_start 必须落在执行段落（Run Executor；executor 为 null 的 phase 为
  // Run Evaluator）标题之后、Agent 调用之前
  const executorHeading = text.search(/### Run (Executor|Evaluator)/);
  if (executorHeading === -1) {
    missing.push('缺少 ### Run Executor / ### Run Evaluator 段落');
  } else if (startIdx < executorHeading) {
    missing.push('phase_start 调用未置于执行段落之后（gate 返回后、执行前）');
  }

  // retry 重跑 / backtrack recall 重进执行前重新调用（executor 为 null 的 phase
  // 仅 backtrack recall 重进语境）
  if (!/Repeat this call/i.test(text)) {
    missing.push(
      '缺少「retry 重跑 / backtrack recall 重进执行前重新调用」文本（Repeat this call …）',
    );
  }
  // Verdict 的 phase_next 不触发 phase_start
  if (!/never triggers one/i.test(text)) {
    missing.push('缺少「Verdict phase_next 不触发」文本（never triggers one）');
  }

  // Verdict 段落内不出现 phase_start 调用
  const verdictIdx = text.search(/### Verdict/i);
  if (verdictIdx !== -1) {
    const verdictSection = text.slice(verdictIdx);
    if (verdictSection.includes('__MCP:phase_start__')) {
      missing.push('Verdict 段落内出现 __MCP:phase_start__ 调用（不应触发）');
    }
  }

  return missing;
}

/**
 * workflow 编排技能协议断言：LOOP 内 gate 通过后、executor 执行前含 phase_start，
 * 且注明每轮迭代一次、done/error 不调用。
 */
function checkWorkflowSkillProtocol(text: string): string[] {
  const missing: string[] = [];

  const startIdx = text.indexOf('__MCP:phase_start__');
  if (startIdx === -1) {
    missing.push('LOOP 内缺少 __MCP:phase_start__ 调用');
    return missing;
  }

  // Start Phase 段位于 Run Executor 之前（gate 通过后、executor 执行前）
  const startPhaseIdx = text.indexOf('-- Start Phase --');
  const runExecutorIdx = text.indexOf('-- Run Executor if Exist --');
  if (startPhaseIdx === -1 || runExecutorIdx === -1) {
    missing.push('缺少 -- Start Phase -- / -- Run Executor if Exist -- 段落');
  } else {
    if (startPhaseIdx > runExecutorIdx) {
      missing.push('phase_start 未置于 -- Run Executor if Exist -- 之前');
    }
    const segment = text.slice(startPhaseIdx, runExecutorIdx);
    if (!segment.includes('__MCP:phase_start__')) {
      missing.push('phase_start 调用不在 -- Start Phase -- 段内');
    }
  }

  // 每轮迭代一次；done / error 路径不调用
  if (!text.includes('每轮迭代 gate 通过后调用一次')) {
    missing.push('缺少「每轮迭代 gate 通过后调用一次」注明');
  }
  if (!text.includes('gate.error / gate.done 路径在到达此处前已离开 LOOP，不调用')) {
    missing.push('缺少「gate.error / gate.done 不调用」注明');
  }

  return missing;
}

// ===========================================================================
// phase 技能协议 (AC-11)
// ===========================================================================

describe('phase 技能 SKILL.md — phase_next → phase_start 协议 (AC-11)', () => {
  it('8 个 phase 技能文本均匹配 phase_next → __MCP:phase_start__({change, phase}) 序列，缺失时列出文件名', () => {
    const failures: string[] = [];
    for (const skill of PHASE_SKILLS) {
      const missing = checkPhaseSkillProtocol(readSkill(skill));
      if (missing.length > 0) {
        failures.push(`${skill}/SKILL.md: ${missing.join('；')}`);
      }
    }
    expect(failures).toEqual([]);
  });

  for (const skill of PHASE_SKILLS) {
    it(`${skill}/SKILL.md: gate 返回本 phase 后、executor 执行前含 phase_start，retry / backtrack recall 重跑重新调用，Verdict 不触发`, () => {
      const missing = checkPhaseSkillProtocol(readSkill(skill));
      expect(missing).toEqual([]);
    });
  }
});

// ===========================================================================
// workflow 编排技能协议 (AC-11)
// ===========================================================================

describe('workflow 编排技能 SKILL.md — LOOP 内 phase_start (AC-11)', () => {
  it('workflow-requirement / workflow-test-only 的 LOOP 内含 phase_start 且注明 done/error 不调用，缺失时列出文件名', () => {
    const failures: string[] = [];
    for (const skill of WORKFLOW_SKILLS) {
      const missing = checkWorkflowSkillProtocol(readSkill(skill));
      if (missing.length > 0) {
        failures.push(`${skill}/SKILL.md: ${missing.join('；')}`);
      }
    }
    expect(failures).toEqual([]);
  });

  for (const skill of WORKFLOW_SKILLS) {
    it(`${skill}/SKILL.md: 每轮迭代 gate 通过后、executor 执行前调用一次 phase_start；done / error 不调用`, () => {
      const missing = checkWorkflowSkillProtocol(readSkill(skill));
      expect(missing).toEqual([]);
    });
  }
});
