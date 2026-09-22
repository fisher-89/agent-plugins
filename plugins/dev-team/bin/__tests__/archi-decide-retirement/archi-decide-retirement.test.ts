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
