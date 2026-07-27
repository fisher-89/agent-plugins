/**
 * 单元测试: runConfigGet — projectRoot 显式注入 (AC-7)
 *
 * @see openspec/changes/mcp-project-root-lock/test-design.md
 */

import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { afterEach, describe, expect, it, vi } from 'vite-plus/test';

import * as projectRootLib from '../lib/project-root';
import { runConfigGet } from './config-get';

interface TempProject {
  root: string;
  cleanup: () => void;
}

function createTempProject(
  config: Record<string, unknown> = { schema: 'spec-driven' },
): TempProject {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'config-get-'));
  fs.mkdirSync(path.join(root, 'openspec'), { recursive: true });
  fs.writeFileSync(path.join(root, 'openspec', 'config.json'), JSON.stringify(config), 'utf-8');
  return {
    root,
    cleanup: () => fs.rmSync(root, { recursive: true, force: true }),
  };
}

describe('runConfigGet — projectRoot 显式注入', () => {
  const spies: Array<ReturnType<typeof vi.spyOn>> = [];

  afterEach(() => {
    for (const spy of spies.splice(0)) {
      spy.mockRestore();
    }
  });

  it('显式 projectRoot 指向含 openspec/config.json 的 fixture：exists === true 且 value 等于写入值；key 回显等于入参 (AC-7)', () => {
    const project = createTempProject({ schema: 'spec-driven', custom: { nested: 42 } });
    try {
      const result = runConfigGet({ key: 'custom.nested', projectRoot: project.root });
      expect(result.key).toBe('custom.nested');
      expect(result.exists).toBe(true);
      expect(result.value).toBe(42);
    } finally {
      project.cleanup();
    }
  });

  it('spy getProjectDir：显式非空 projectRoot 时调用次数为 0 (AC-7)', () => {
    const project = createTempProject();
    const spy = vi.spyOn(projectRootLib, 'getProjectDir');
    spies.push(spy);
    try {
      runConfigGet({ key: 'schema', projectRoot: project.root });
      expect(spy).toHaveBeenCalledTimes(0);
    } finally {
      project.cleanup();
    }
  });

  it('省略 projectRoot / projectRoot: "" / undefined：均调用 getProjectDir，三者行为一致（证明 || 非 ??）(AC-7)', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      context: 'from-getProjectDir',
    });
    const spy = vi.spyOn(projectRootLib, 'getProjectDir').mockReturnValue(project.root);
    spies.push(spy);
    try {
      spy.mockClear();
      const a = runConfigGet({ key: 'context' });
      expect(spy).toHaveBeenCalled();
      const callsA = spy.mock.calls.length;

      spy.mockClear();
      const b = runConfigGet({ key: 'context', projectRoot: '' });
      expect(spy).toHaveBeenCalled();
      const callsB = spy.mock.calls.length;

      spy.mockClear();
      const c = runConfigGet({ key: 'context', projectRoot: undefined });
      expect(spy).toHaveBeenCalled();
      const callsC = spy.mock.calls.length;

      expect(callsA).toBeGreaterThan(0);
      expect(callsB).toBeGreaterThan(0);
      expect(callsC).toBeGreaterThan(0);
      expect(a.value).toBe('from-getProjectDir');
      expect(b.value).toBe(a.value);
      expect(c.value).toBe(a.value);
    } finally {
      project.cleanup();
    }
  });

  it('键不存在：exists === false，value 按实现为 undefined/null，不得抛', () => {
    const project = createTempProject();
    try {
      expect(() => runConfigGet({ key: 'missing.key', projectRoot: project.root })).not.toThrow();
      const result = runConfigGet({ key: 'missing.key', projectRoot: project.root });
      expect(result.exists).toBe(false);
      expect(result.value === undefined || result.value === null).toBe(true);
    } finally {
      project.cleanup();
    }
  });

  it('key 为空串 / 超长 / 含 . 嵌套路径：行为明确且不崩溃', () => {
    const project = createTempProject({ a: { b: 'ok' } });
    try {
      expect(() => runConfigGet({ key: '', projectRoot: project.root })).not.toThrow();
      expect(() =>
        runConfigGet({ key: 'k'.repeat(1001), projectRoot: project.root }),
      ).not.toThrow();
      const nested = runConfigGet({ key: 'a.b', projectRoot: project.root });
      expect(nested.exists).toBe(true);
      expect(nested.value).toBe('ok');
    } finally {
      project.cleanup();
    }
  });

  it('显式根 !== cwd 时读取的是该根配置，而非 cwd 配置', () => {
    const project = createTempProject({
      schema: 'spec-driven',
      context: 'temp-only-value',
    });
    expect(path.resolve(project.root)).not.toBe(path.resolve(process.cwd()));
    try {
      const result = runConfigGet({ key: 'context', projectRoot: project.root });
      expect(result.value).toBe('temp-only-value');
    } finally {
      project.cleanup();
    }
  });
});
