/**
 * 集成测试: expandIncludes → applyEnvTokens → assertNoNameTokens 管道
 *
 * @see openspec/changes/cursor-omit-subagent-stop/test-design.md
 */

import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { afterEach, describe, expect, it, vi, type Mock } from 'vite-plus/test';

import { applyEnvTokens } from '../../apply-env-tokens';
import { assertNoNameTokens } from '../../assert-no-tokens';
import { getEnv } from '../../env';
import { expandIncludes } from '../../expand-includes';

const tempRoots: string[] = [];
let cwdSpy: Mock | undefined;

function mockCwd(root: string): void {
  cwdSpy?.mockRestore();
  cwdSpy = vi.spyOn(process, 'cwd').mockReturnValue(root);
}

function runPipeline(text: string, env = getEnv('cursor')): string {
  const expanded = expandIncludes(text, env);
  return applyEnvTokens(expanded, env);
}

afterEach(() => {
  cwdSpy?.mockRestore();
  cwdSpy = undefined;
  while (tempRoots.length > 0) {
    const dir = tempRoots.pop();
    if (dir) rmSync(dir, { recursive: true, force: true });
  }
});

describe('管道顺序与残留失败', () => {
  it('完整三段管道后无 __INCLUDE: 且 __BIN:cli__ 已展开', () => {
    const root = mkdtempSync(join(tmpdir(), 'pipeline-'));
    tempRoots.push(root);
    mkdirSync(join(root, '_fragments'), { recursive: true });
    writeFileSync(
      join(root, '_fragments/static-analysis-gate.cursor.md'),
      'run_static_analysis via __BIN:cli__\n',
    );
    mockCwd(root);

    const host = 'before __INCLUDE:static-analysis-gate__ after';
    const final = runPipeline(host);
    expect(final).toContain('run_static_analysis');
    expect(final).toContain('cli.cjs');
    expect(final).not.toContain('__INCLUDE:');
    expect(final).not.toContain('__BIN:');

    mkdirSync(join(root, 'out'), { recursive: true });
    writeFileSync(join(root, 'out/result.md'), final);
    expect(() => assertNoNameTokens(join(root, 'out'), getEnv('cursor'))).not.toThrow();
  });

  it('跳过 expand 直接 assert 含 __INCLUDE: 的文本应抛错', () => {
    const root = mkdtempSync(join(tmpdir(), 'pipeline-skip-'));
    tempRoots.push(root);
    mkdirSync(join(root, 'out'), { recursive: true });
    writeFileSync(join(root, 'out/bad.md'), '__INCLUDE:static-analysis-gate__');
    expect(() => assertNoNameTokens(join(root, 'out'), getEnv('claude'))).toThrow(
      /unresolved name-class tokens/,
    );
  });

  it('环状 fragment 在 expand 阶段抛错', () => {
    const root = mkdtempSync(join(tmpdir(), 'pipeline-cycle-'));
    tempRoots.push(root);
    mkdirSync(join(root, '_fragments'), { recursive: true });
    writeFileSync(join(root, '_fragments/a.md'), '__INCLUDE:b__');
    writeFileSync(join(root, '_fragments/b.md'), '__INCLUDE:a__');
    mockCwd(root);
    expect(() => runPipeline('__INCLUDE:a__')).toThrow(/Include cycle detected/);
  });

  it('空 default include + 后续 env token：Claude 嵌入空串且其余 token 仍被 apply', () => {
    const root = mkdtempSync(join(tmpdir(), 'pipeline-claude-empty-'));
    tempRoots.push(root);
    mkdirSync(join(root, '_fragments'), { recursive: true });
    writeFileSync(join(root, '_fragments/static-analysis-gate.md'), '');
    mockCwd(root);

    const text = 'head __INCLUDE:static-analysis-gate__ tail __BIN:cli__';
    const final = runPipeline(text, getEnv('claude'));
    expect(final).toBe('head  tail cli.cjs');
    expect(final).not.toContain('__INCLUDE:');
  });

  it('深度恰好 16 成功、17 失败', () => {
    const root = mkdtempSync(join(tmpdir(), 'pipeline-depth-'));
    tempRoots.push(root);
    mkdirSync(join(root, '_fragments'), { recursive: true });
    for (let i = 0; i < 16; i++) {
      const next = i + 1;
      const body = next < 16 ? `__INCLUDE:chain-${next}__` : 'leaf';
      writeFileSync(join(root, '_fragments', `chain-${i}.md`), body);
    }
    mockCwd(root);

    expect(runPipeline('__INCLUDE:chain-0__')).toBe('leaf');
    expect(() => expandIncludes('__INCLUDE:chain-0__', getEnv('cursor'), [], 17)).toThrow(
      /Include depth exceeded/,
    );
  });
});
