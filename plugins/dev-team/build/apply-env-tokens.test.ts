/**
 * 单元测试: apply-env-tokens.ts — 名称/路径 token 展开
 */

import { describe, expect, it } from 'vite-plus/test';

import { applyEnvTokens } from './apply-env-tokens';
import { getEnv, type ProductEnv } from './env';

describe('applyEnvTokens', () => {
  it('claude/cursor env 将 __MCP:phase_log__ 展开为插件 MCP 前缀', () => {
    for (const key of ['claude', 'cursor'] as const) {
      const env = getEnv(key);
      expect(applyEnvTokens('call __MCP:phase_log__', env)).toBe(
        'call mcp__plugin_dev-team_dev-team__phase_log',
      );
    }
  });

  it('cursorHome env 将 __MCP:phase_log__ 展开为 user MCP 前缀', () => {
    const env = getEnv('cursorHome');
    expect(applyEnvTokens('call __MCP:phase_log__', env)).toBe(
      'call mcp__user-dev-team_mcp__phase_log',
    );
  });

  it('cursorHome 展开 __SKILL: 与 __BIN: 带 dev-team_ 前缀', () => {
    const env = getEnv('cursorHome');
    expect(applyEnvTokens('__SKILL:explore__', env)).toBe('dev-team_explore');
    expect(applyEnvTokens('__BIN:mcp__', env)).toBe('dev-team_mcp.cjs');
  });

  it('plugin env 展开 __BIN:hooks__ 与 __CALL_SKILL:propose__', () => {
    const env = getEnv('cursor');
    expect(applyEnvTokens('__BIN:hooks__', env)).toBe('hooks.cjs');
    expect(applyEnvTokens('__CALL_SKILL:propose__', env)).toBe('dev-team:propose');
  });

  it('cursorHome 展开 __AGENT: 与 __CALL_AGENT: 带下划线前缀', () => {
    const env = getEnv('cursorHome');
    expect(applyEnvTokens('__AGENT:proposal-planner__', env)).toBe('dev-team_proposal-planner');
    expect(applyEnvTokens('__CALL_AGENT:proposal-planner__', env)).toBe(
      'dev-team_proposal-planner',
    );
  });

  it('plugin env 默认展开路径 token 为 contentRoot / runtimeRoot', () => {
    const env = getEnv('claude');
    const text = 'root=__DEV_TEAM_ROOT__;';
    expect(applyEnvTokens(text, env)).toBe(`root=${env.pluginRoot};`);
  });

  it('text 为 null / undefined 时抛错', () => {
    const env = getEnv('claude');
    expect(() => applyEnvTokens(null as unknown as string, env)).toThrow();
    expect(() => applyEnvTokens(undefined as unknown as string, env)).toThrow();
  });

  it('env 为 null / undefined 时抛错', () => {
    expect(() => applyEnvTokens('x', null as unknown as ProductEnv)).toThrow();
    expect(() => applyEnvTokens('x', undefined as unknown as ProductEnv)).toThrow();
  });

  it('text 为空字符串时返回空字符串', () => {
    expect(applyEnvTokens('', getEnv('claude'))).toBe('');
  });

  it('超长文本含多枚 token 时全部正确展开且不崩溃', () => {
    const env = getEnv('cursor');
    const chunk = '__SKILL:explore__ __MCP:phase_log__ __BIN:cli__ ';
    const text = chunk.repeat(80);
    expect(text.length).toBeGreaterThan(1000);
    const result = applyEnvTokens(text, env);
    expect(result).not.toContain('__SKILL:');
    expect(result).not.toContain('__MCP:');
    expect(result).not.toContain('__BIN:');
    expect(result).toContain('explore');
    expect(result).toContain('mcp__plugin_dev-team_dev-team__phase_log');
    expect(result).toContain('cli.cjs');
  });

  it('特殊字符夹杂 token 时仅替换 token、其余保留', () => {
    const env = getEnv('claude');
    const text = 'a\nb__MCP:phase_log__c\0d🙂e';
    expect(applyEnvTokens(text, env)).toBe('a\nbmcp__plugin_dev-team_dev-team__phase_logc\0d🙂e');
  });

  it('无 token 的普通字符串原样返回', () => {
    const plain = 'hello world without tokens';
    expect(applyEnvTokens(plain, getEnv('cursor'))).toBe(plain);
  });

  it('非法或残缺占位不误展开', () => {
    const env = getEnv('cursor');
    expect(applyEnvTokens('__MCP:__', env)).toBe('__MCP:__');
    expect(applyEnvTokens('__SKILL:BadId__', env)).toBe('__SKILL:BadId__');
    expect(applyEnvTokens('__AGENT:foo', env)).toBe('__AGENT:foo');
  });

  it('不发明裸服务器级 MCP 引用，仅替换合法 __MCP:<tool>__', () => {
    const env = getEnv('cursor');
    const bare = 'mcp__plugin_dev-team_dev-team__';
    expect(applyEnvTokens(bare, env)).toBe(bare);
    expect(applyEnvTokens('use __MCP:phase_log__ please', env)).toBe(
      'use mcp__plugin_dev-team_dev-team__phase_log please',
    );
  });

  it('强制展开路径 token（安装期）', () => {
    const home = getEnv('cursorHome');
    expect(applyEnvTokens('__DEV_TEAM_ROOT__/x', home)).toBe(`${home.pluginRoot}/x`);
    expect(applyEnvTokens('__DEV_TEAM_ROOT__/y', home)).toBe(`${home.pluginRoot}/y`);
  });
});
