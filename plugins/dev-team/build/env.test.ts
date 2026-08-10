/**
 * 单元测试: env.ts — 三产物 ProductEnv 表
 */

import { describe, expect, it } from 'vite-plus/test';

import { getEnv, PRODUCT_ENV_KEYS, type ProductEnvKey } from './env';

describe('getEnv', () => {
  it('claude / cursor / cursorHome 均返回对象且 key 与入参一致', () => {
    for (const key of PRODUCT_ENV_KEYS) {
      const env = getEnv(key);
      expect(env).toBeTypeOf('object');
      expect(env.key).toBe(key);
    }
  });

  it('cursorHome 前缀与 hooks/path 阶段字段符合 home 产物约定', () => {
    const env = getEnv('cursorHome');
    expect(env.mcpToolPrefix).toBe('mcp__user-dev-team_mcp__');
    expect(env.namePrefix).toBe('dev-team_');
    expect(env.pluginPrefix).toBe('');
    expect(env.hooksFilePath).toBe('hooks.json');
    expect(env.pathReplacePhase).toBe('install');
  });

  it('plugin 行 mcp 前缀为现网插件前缀', () => {
    for (const key of ['claude', 'cursor'] as const) {
      const env = getEnv(key);
      expect(env.mcpToolPrefix).toBe('mcp__plugin_dev-team_dev-team__');
      expect(env.namePrefix).toBe('');
    }
  });

  it('三行 outDir 分别指向 claude-plugins / cursor-plugins / cursor-home-image 下 dev-team', () => {
    expect(getEnv('claude').outDir.replace(/\\/g, '/')).toContain('claude-plugins/dev-team');
    expect(getEnv('cursor').outDir.replace(/\\/g, '/')).toContain('cursor-plugins/dev-team');
    expect(getEnv('cursorHome').outDir.replace(/\\/g, '/')).toContain('cursor-home-image/dev-team');
  });

  it('非法 key 运行时抛错', () => {
    expect(() => getEnv('vscode' as ProductEnvKey)).toThrow(/Unknown product env key/);
  });

  it('key 为 null / undefined 时抛错', () => {
    expect(() => getEnv(null as unknown as ProductEnvKey)).toThrow();
    expect(() => getEnv(undefined as unknown as ProductEnvKey)).toThrow();
  });

  it('key 为空字符串时抛错', () => {
    expect(() => getEnv('' as ProductEnvKey)).toThrow();
  });
});

describe('PRODUCT_ENV_KEYS', () => {
  it('长度为 3 且含且仅含 claude / cursor / cursorHome', () => {
    expect(PRODUCT_ENV_KEYS).toHaveLength(3);
    expect([...PRODUCT_ENV_KEYS].sort()).toEqual(['claude', 'cursor', 'cursorHome'].sort());
  });
});
