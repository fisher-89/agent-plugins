/**
 * 集成测试: Agent / 产物下线契约
 *
 * @see openspec/changes/migrate-archi-decide-to-mcp/test-design.md
 */

import * as fs from 'node:fs';
import * as path from 'node:path';

import { describe, expect, it } from 'vite-plus/test';

const REPO_ROOT = path.resolve(import.meta.dirname, '../../../../..');

describe('architecture.md 仅 MCP (AC-08)', () => {
  const architectureMd = path.join(REPO_ROOT, 'plugins', 'dev-team', 'agents', 'architecture.md');

  it('含 archi_decide 与 Decide 步骤 MCP 指引', () => {
    const content = fs.readFileSync(architectureMd, 'utf-8');
    expect(content).toContain('archi_decide');
    expect(content).toMatch(/decide.*MCP|MCP.*archi_decide/i);
  });

  it('全文不出现 archi-decide.py 与 python .../archi-decide', () => {
    const content = fs.readFileSync(architectureMd, 'utf-8');
    expect(content).not.toContain('archi-decide.py');
    expect(content).not.toMatch(/python\s+.*archi-decide/i);
  });

  it('工具清单段不再列出 __DEV_TEAM_ROOT__/utils/archi-decide.py', () => {
    const content = fs.readFileSync(architectureMd, 'utf-8');
    expect(content).not.toContain('utils/archi-decide.py');
    expect(content).not.toContain('__DEV_TEAM_ROOT__/utils/archi-decide.py');
  });
});

describe('Python 脚本与 manifest 下线 (AC-09)', () => {
  const sourcePy = path.join(REPO_ROOT, 'plugins', 'dev-team', 'utils', 'archi-decide.py');
  const productRoots = [
    path.join(REPO_ROOT, 'claude-plugins', 'dev-team'),
    path.join(REPO_ROOT, 'cursor-plugins', 'dev-team'),
    path.join(REPO_ROOT, 'cursor-home-image', 'dev-team'),
  ];
  const existingProductRoots = productRoots.filter((root) => fs.existsSync(root));
  const buildProductsExist = existingProductRoots.length > 0;
  const productSkipReason =
    '构建产物未生成：claude-plugins/cursor-plugins/cursor-home-image 目录均不存在，跳过产物断言';

  it('plugins/dev-team/utils/archi-decide.py 不存在', () => {
    expect(fs.existsSync(sourcePy)).toBe(false);
  });

  it('已构建时三端插件目录均无 utils/archi-decide.py', (ctx) => {
    if (!buildProductsExist) {
      ctx.skip(productSkipReason);
      return;
    }
    for (const root of existingProductRoots) {
      const pyPath = path.join(root, 'utils', 'archi-decide.py');
      expect(fs.existsSync(pyPath)).toBe(false);
    }
  });

  it('manifest.json 无 archi-decide.py 条目', (ctx) => {
    const manifestPath = path.join(REPO_ROOT, 'cursor-home-image', 'dev-team', 'manifest.json');
    if (!fs.existsSync(manifestPath)) {
      ctx.skip(`${productSkipReason}（manifest.json 不存在）`);
      return;
    }
    const manifest = fs.readFileSync(manifestPath, 'utf-8');
    expect(manifest).not.toContain('archi-decide.py');
  });
});
