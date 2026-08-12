/**
 * 单元测试: assert-no-tokens.ts — 名称类 token 残留断言
 */

import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { afterEach, describe, expect, it } from 'vite-plus/test';

import { assertNoNameTokens } from './assert-no-tokens';
import { getEnv, type ProductEnv } from './env';

const tempDirs: string[] = [];

function makeTempDir(): string {
  const dir = mkdtempSync(join(tmpdir(), 'assert-tokens-'));
  tempDirs.push(dir);
  return dir;
}

afterEach(() => {
  while (tempDirs.length > 0) {
    const dir = tempDirs.pop();
    if (dir) rmSync(dir, { recursive: true, force: true });
  }
});

describe('assertNoNameTokens', () => {
  it('临时目录仅含已展开文本时不抛错', () => {
    const root = makeTempDir();
    writeFileSync(join(root, 'ok.md'), 'use mcp__plugin_dev-team_dev-team__phase_log\n');
    expect(() => assertNoNameTokens(root, getEnv('claude'))).not.toThrow();
  });

  it('任意 env 下文件含名称类 token 残留时抛错', () => {
    const root = makeTempDir();
    writeFileSync(join(root, 'bad.md'), 'call __MCP:phase_log__\n');
    for (const key of ['claude', 'cursor', 'cursorHome'] as const) {
      expect(() => assertNoNameTokens(root, getEnv(key))).toThrow(/unresolved name-class tokens/);
    }
  });

  it('rootDir 不存在时抛错', () => {
    expect(() =>
      assertNoNameTokens(join(tmpdir(), 'missing-assert-root-' + Date.now()), getEnv('claude')),
    ).toThrow();
  });

  it('rootDir / env 为 null / undefined 时抛错', () => {
    expect(() => assertNoNameTokens(null as unknown as string, getEnv('claude'))).toThrow(
      'rootDir is required',
    );
    expect(() => assertNoNameTokens(undefined as unknown as string, getEnv('claude'))).toThrow(
      'rootDir is required',
    );
    const root = makeTempDir();
    expect(() => assertNoNameTokens(root, null as unknown as ProductEnv)).toThrow();
    expect(() => assertNoNameTokens(root, undefined as unknown as ProductEnv)).toThrow();
  });

  it('rootDir 为空字符串时抛错或拒绝', () => {
    expect(() => assertNoNameTokens('', getEnv('claude'))).toThrow('rootDir is required');
  });

  it('空目录不抛错', () => {
    const root = makeTempDir();
    expect(() => assertNoNameTokens(root, getEnv('cursor'))).not.toThrow();
  });

  it('仅二进制 / 排除扩展名文件含疑似字节时不误报', () => {
    const root = makeTempDir();
    writeFileSync(join(root, 'openspec-bundled.js'), '__MCP:phase_log__');
    writeFileSync(join(root, 'x.map'), '__SKILL:explore__');
    writeFileSync(join(root, 'pic.png'), Buffer.from('__AGENT:foo__'));
    expect(() => assertNoNameTokens(root, getEnv('claude'))).not.toThrow();
  });

  it('plugin env 下残留路径 token 视为失败', () => {
    const root = makeTempDir();
    mkdirSync(join(root, 'bin'), { recursive: true });
    writeFileSync(join(root, 'bin', 'hooks.cjs'), 'const r = "__DEV_TEAM_ROOT__";\n');
    expect(() => assertNoNameTokens(root, getEnv('claude'))).toThrow(/unresolved/);
    expect(() => assertNoNameTokens(root, getEnv('cursor'))).toThrow(/unresolved/);
  });

  it('文件含合法残留 __INCLUDE:static-analysis-gate__ 时抛错且 message 可定位文件', () => {
    const root = makeTempDir();
    writeFileSync(join(root, 'agent.md'), 'step __INCLUDE:static-analysis-gate__\n');
    expect(() => assertNoNameTokens(root, getEnv('claude'))).toThrow(
      /unresolved name-class tokens[\s\S]*agent\.md/,
    );
  });

  it('文件仅含子串 __INCLUDE:（非法/残缺 token）时亦抛错', () => {
    const root = makeTempDir();
    writeFileSync(join(root, 'broken.md'), 'partial __INCLUDE: token\n');
    expect(() => assertNoNameTokens(root, getEnv('cursor'))).toThrow(
      /unresolved name-class tokens/,
    );
  });

  it('env={}（空对象，缺失必填字段）时抛错', () => {
    const root = makeTempDir();
    writeFileSync(join(root, 'ok.md'), 'clean');
    expect(() => assertNoNameTokens(root, {} as ProductEnv)).toThrow();
  });

  it('超长文件（>1000 chars）末尾残留 __INCLUDE: 仍能检出', () => {
    const root = makeTempDir();
    const padding = 'x'.repeat(1000);
    writeFileSync(join(root, 'long.md'), `${padding}__INCLUDE:tail__`);
    expect(() => assertNoNameTokens(root, getEnv('claude'))).toThrow(/long\.md/);
  });

  it('多文件仅一处残留时抛错列表包含该相对路径', () => {
    const root = makeTempDir();
    writeFileSync(join(root, 'clean.md'), 'ok');
    writeFileSync(join(root, 'dirty.md'), '__INCLUDE:bad__');
    expect(() => assertNoNameTokens(root, getEnv('cursorHome'))).toThrow(/dirty\.md/);
  });

  it('env 含多余字段时无残留 token 的目录仍不抛错', () => {
    const root = makeTempDir();
    writeFileSync(join(root, 'ok.md'), 'expanded content');
    const env = { ...getEnv('claude'), extra: true } as ProductEnv;
    expect(() => assertNoNameTokens(root, env)).not.toThrow();
  });
});
