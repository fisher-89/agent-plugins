/**
 * 集成测试: implementation-generator / implementation-evaluator 静态定义
 *           + test-execution-executor / test-execution-evaluator 静态定义 (AC-8/AC-9)
 *
 * 覆盖 AC-8、AC-9：移除静态检查步骤与 I7 检查项；验证 test-execution 相关 agent/skill 文件
 *
 * @see openspec/changes/static-check-agent-hook/test-design.md
 * @see openspec/changes/consolidate-test-execution-phase/test-design.md
 */

import * as fs from 'node:fs';
import * as path from 'node:path';

import { describe, expect, it } from 'vite-plus/test';

// ---------------------------------------------------------------------------
// 路径
// ---------------------------------------------------------------------------

/** vitest CWD 为 plugins/dev-team/bin/ */
const projectRoot = path.resolve(process.cwd(), '../../..');
const generatorPath = path.resolve(
  projectRoot,
  'plugins/dev-team/agents/implementation-generator.md',
);

function readAgent(filePath: string): string {
  return fs.readFileSync(filePath, 'utf-8');
}

// ---------------------------------------------------------------------------
// AC-8: implementation-generator.md
// ---------------------------------------------------------------------------

describe('implementation-generator.md — 静态检查移除 (AC-8)', () => {
  it('Process 不应含 config_get、static_analysis、步骤 7/8', () => {
    const content = readAgent(generatorPath);
    const processSection = content.split('## Process')[1]?.split('## Output')[0] ?? '';
    expect(processSection).not.toMatch(/config_get/i);
    expect(processSection).not.toMatch(/static_analysis/i);
    expect(processSection).not.toMatch(/^7\./m);
    expect(processSection).not.toMatch(/^8\./m);
  });

  it('Output 不应含 reports/static_analysis.json 及 JSON 报告模板', () => {
    const content = readAgent(generatorPath);
    const outputSection = content.split('## Output')[1]?.split('## Constraints')[0] ?? '';
    expect(outputSection).not.toContain('reports/static_analysis.json');
    expect(outputSection).not.toMatch(/"phase":\s*"implement"/);
  });

  it('frontmatter description 不应含 static-check / static_analysis 引用', () => {
    const content = readAgent(generatorPath);
    const frontmatter = content.match(/^---[\s\S]*?---/)?.[0] ?? '';
    expect(frontmatter).not.toMatch(/static-check|static_analysis/i);
  });

  it('Process 最后一步应为标记 tasks.md 完成（原步骤 6）', () => {
    const content = readAgent(generatorPath);
    const processSection = content.split('## Process')[1]?.split('## Output')[0] ?? '';
    const steps = [...processSection.matchAll(/^\d+\.\s+(.+)$/gm)].map((m) => m[1]);
    expect(steps.length).toBeGreaterThan(0);
    const lastStep = steps[steps.length - 1] ?? '';
    expect(lastStep).toMatch(/tasks\.md|标记.*完成|\[x\]/i);
  });
});

// ===========================================================================
// AC-8: Agent 文件存在性验证
// ===========================================================================

describe('Agent 文件存在性验证 — AC-8', () => {
  const agentDir = path.resolve(projectRoot, 'plugins/dev-team/agents');

  it('test-execution-executor.md 应存在', () => {
    const filePath = path.join(agentDir, 'test-execution-executor.md');
    expect(fs.existsSync(filePath)).toBe(true);
  });

  it('test-execution-evaluator.md 应存在', () => {
    const filePath = path.join(agentDir, 'test-execution-evaluator.md');
    expect(fs.existsSync(filePath)).toBe(true);
  });

  it('integration-test-executor.md 不应存在（已合并到 test-execution）', () => {
    const filePath = path.join(agentDir, 'integration-test-executor.md');
    expect(fs.existsSync(filePath)).toBe(false);
  });

  it('integration-test-evaluator.md 不应存在（已合并到 test-execution）', () => {
    const filePath = path.join(agentDir, 'integration-test-evaluator.md');
    expect(fs.existsSync(filePath)).toBe(false);
  });

  it('unit-test-executor.md 不应存在（已重命名为 test-execution-executor.md）', () => {
    const filePath = path.join(agentDir, 'unit-test-executor.md');
    expect(fs.existsSync(filePath)).toBe(false);
  });

  it('unit-test-evaluator.md 不应存在（已重命名为 test-execution-evaluator.md）', () => {
    const filePath = path.join(agentDir, 'unit-test-evaluator.md');
    expect(fs.existsSync(filePath)).toBe(false);
  });
});

describe('Agent 内容验证 — AC-8', () => {
  const agentDir = path.resolve(projectRoot, 'plugins/dev-team/agents');

  it('test-execution-executor.md 应包含 test-execution 引用', () => {
    const content = readAgent(path.join(agentDir, 'test-execution-executor.md'));
    // 检查文件包含 test-execution 相关描述
    expect(content).toMatch(/test-execution/i);
  });

  it('test-execution-executor.md frontmatter description 应包含 test execution 描述', () => {
    const content = readAgent(path.join(agentDir, 'test-execution-executor.md'));
    const description = content.match(/description:\s*\|?\s*(.+)/)?.[1] ?? '';
    expect(description).toMatch(/test execution|execution report|diagnostic|CLI/i);
  });

  it('test-execution-evaluator.md 不应含 unit-test 引用', () => {
    const content = readAgent(path.join(agentDir, 'test-execution-evaluator.md'));
    expect(content).not.toMatch(/unit-test/i);
  });
});

// ===========================================================================
// AC-9: Skill 文件存在性验证
// ===========================================================================

describe('Skill 文件存在性验证 — AC-9', () => {
  const skillsDir = path.resolve(projectRoot, 'plugins/dev-team/skills');

  it('skills/phase-test-execution/SKILL.md 应存在', () => {
    const filePath = path.join(skillsDir, 'phase-test-execution', 'SKILL.md');
    expect(fs.existsSync(filePath)).toBe(true);
  });

  it('skills/phase-integration-test/ 目录不应存在（已合并到 phase-test-execution）', () => {
    const dirPath = path.join(skillsDir, 'phase-integration-test');
    expect(fs.existsSync(dirPath)).toBe(false);
  });

  it('skills/phase-unit-test/ 目录不应存在（已重命名为 phase-test-execution）', () => {
    const dirPath = path.join(skillsDir, 'phase-unit-test');
    expect(fs.existsSync(dirPath)).toBe(false);
  });
});

describe('Skill 内容验证 — AC-9', () => {
  const skillsDir = path.resolve(projectRoot, 'plugins/dev-team/skills');

  it('phase-test-execution/SKILL.md 不应含 unit-test 引用', () => {
    const filePath = path.join(skillsDir, 'phase-test-execution', 'SKILL.md');
    if (fs.existsSync(filePath)) {
      const content = readAgent(filePath);
      expect(content).not.toMatch(/unit-test/i);
    }
  });

  it('phase-test-execution/SKILL.md 不应含 integration-test 引用', () => {
    const filePath = path.join(skillsDir, 'phase-test-execution', 'SKILL.md');
    if (fs.existsSync(filePath)) {
      const content = readAgent(filePath);
      expect(content).not.toMatch(/integration-test/i);
    }
  });
});
