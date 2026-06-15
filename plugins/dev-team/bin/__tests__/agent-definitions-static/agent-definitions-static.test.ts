/**
 * 集成测试: implementation-generator / implementation-evaluator 静态定义
 *
 * 覆盖 AC-8、AC-9：移除静态检查步骤与 I7 检查项。
 *
 * @see openspec/changes/static-check-agent-hook/test-design.md
 */

import * as fs from 'node:fs';
import * as path from 'node:path';

import { describe, expect, it } from 'vite-plus/test';

// ---------------------------------------------------------------------------
// 路径
// ---------------------------------------------------------------------------

/** vitest CWD 为 plugins/dev-team/bin/ */
const projectRoot = path.resolve(process.cwd(), '../../..');
const generatorPath = path.resolve(projectRoot, 'plugins/dev-team/agents/implementation-generator.md');
const evaluatorPath = path.resolve(projectRoot, 'plugins/dev-team/agents/implementation-evaluator.md');

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
    expect(outputSection).not.toMatch(/"phase":\s*"05-implement"/);
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
