/**
 * 单元测试: lib/session-registry.ts — `session_id → change` 绑定注册表
 *
 * 覆盖范围（openspec/changes/workflow-file-inventory/test-design.md AC-3）:
 * - bindSession 建立/更新绑定并刷新 updatedAt；顺带清理 TTL（24h）过期条目
 * - lookupChange 查绑定；未命中 / 注册表缺失 / 非法 → 返回 null 不抛错
 * - 注册表落盘于 <tmpdir>/dev-team-hooks/<sha256(projectRoot) 前 16 位>.json
 *
 * Mock：vi.mock node:os tmpdir 指向临时目录（隔离真实系统临时目录；
 * 不使用 process.chdir，遵循 Stryker worker 线程约定）；文件系统走真实 fs。
 */

import { createHash } from 'node:crypto';
import * as fs from 'node:fs';
import * as os from 'node:os';
import * as path from 'node:path';

import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test';

const mockTmp = vi.hoisted(() => ({ dir: null as string | null }));

vi.mock('node:os', async (importOriginal) => {
  const actual = await importOriginal<{ tmpdir: () => string }>();
  return { ...actual, tmpdir: () => mockTmp.dir ?? actual.tmpdir() };
});

import { bindSession, lookupChange } from './session-registry';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

let tmpBase = '';

/** 注册表文件的期望落盘路径（与实现同构：<tmpdir>/dev-team-hooks/<hash 前 16 位>.json）。 */
function expectedRegistryPath(projectRoot: string): string {
  const hash = createHash('sha256').update(path.resolve(projectRoot)).digest('hex').slice(0, 16);
  return path.join(tmpBase, 'dev-team-hooks', `${hash}.json`);
}

function readRegistryRaw(projectRoot: string): string | null {
  const filePath = expectedRegistryPath(projectRoot);
  if (!fs.existsSync(filePath)) return null;
  return fs.readFileSync(filePath, 'utf-8');
}

function writeRegistryRaw(projectRoot: string, content: string): void {
  const filePath = expectedRegistryPath(projectRoot);
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, content, 'utf-8');
}

const ROOT = path.resolve(os.tmpdir(), `session-registry-root-${Date.now()}`);
const OTHER_ROOT = `${ROOT}-other`;

beforeEach(() => {
  mockTmp.dir = fs.mkdtempSync(path.join(os.tmpdir(), 'session-registry-test-'));
  tmpBase = mockTmp.dir;
});

afterEach(() => {
  if (mockTmp.dir) {
    fs.rmSync(mockTmp.dir, { recursive: true, force: true });
    mockTmp.dir = null;
  }
  fs.rmSync(ROOT, { recursive: true, force: true });
  fs.rmSync(OTHER_ROOT, { recursive: true, force: true });
});

// ===========================================================================
// bindSession + lookupChange — 正向
// ===========================================================================

describe('bindSession + lookupChange — 正向', () => {
  it('bind(s1, change-a) 后 lookup(s1) 返回 change-a (AC-3)', () => {
    bindSession(ROOT, 'session-1', 'change-a');
    expect(lookupChange(ROOT, 'session-1')).toBe('change-a');
  });

  it('同 session 先绑 change-a 再绑 change-b → lookup 返回 change-b 且 updatedAt 刷新', () => {
    bindSession(ROOT, 'session-1', 'change-a');
    const first = JSON.parse(readRegistryRaw(ROOT)!) as Record<string, { updatedAt: string }>;

    bindSession(ROOT, 'session-1', 'change-b');
    const second = JSON.parse(readRegistryRaw(ROOT)!) as Record<string, { updatedAt: string }>;

    expect(lookupChange(ROOT, 'session-1')).toBe('change-b');
    expect(second['session-1'].updatedAt >= first['session-1'].updatedAt).toBe(true);
  });

  it('s1→change-a、s2→change-b 并存，各自命中互不串账 (AC-3)', () => {
    bindSession(ROOT, 's1', 'change-a');
    bindSession(ROOT, 's2', 'change-b');

    expect(lookupChange(ROOT, 's1')).toBe('change-a');
    expect(lookupChange(ROOT, 's2')).toBe('change-b');
  });

  it('注册表落盘于 tmpdir/dev-team-hooks/<sha256(projectRoot) 前 16 位>.json；不同 projectRoot 哈希隔离互不可见', () => {
    bindSession(ROOT, 's1', 'change-a');
    bindSession(OTHER_ROOT, 's1', 'change-b');

    // 文件位置契约
    expect(fs.existsSync(expectedRegistryPath(ROOT))).toBe(true);
    expect(fs.existsSync(expectedRegistryPath(OTHER_ROOT))).toBe(true);

    // 注册表按根隔离：同一 session 在两个根下绑定不同 change
    expect(lookupChange(ROOT, 's1')).toBe('change-a');
    expect(lookupChange(OTHER_ROOT, 's1')).toBe('change-b');

    // 另一个根的注册表不含本根条目
    const raw = JSON.parse(readRegistryRaw(ROOT)!) as Record<string, unknown>;
    expect(Object.keys(raw)).toEqual(['s1']);
  });
});

// ===========================================================================
// lookupChange — 异常与边界
// ===========================================================================

describe('lookupChange — 异常与边界', () => {
  it('未绑定 session → 返回 null 且不抛错', () => {
    bindSession(ROOT, 's1', 'change-a');
    expect(lookupChange(ROOT, 'never-bound')).toBeNull();
  });

  it('注册表文件不存在 → 返回 null 不抛错', () => {
    expect(readRegistryRaw(ROOT)).toBeNull();
    expect(lookupChange(ROOT, 's1')).toBeNull();
  });

  it('注册表文件 JSON 非法 / 根非对象 → 返回 null 不抛错', () => {
    for (const raw of ['{broken json', '[1, 2]', '"a string"', '42', 'null']) {
      writeRegistryRaw(ROOT, raw);
      expect(lookupChange(ROOT, 's1')).toBeNull();
    }
  });

  it('注册表条目缺 change 字段（结构损坏）时该条目被忽略 → 返回 null', () => {
    writeRegistryRaw(ROOT, JSON.stringify({ s1: { updatedAt: new Date().toISOString() } }));
    expect(lookupChange(ROOT, 's1')).toBeNull();
  });

  it('sessionId 为空字符串 → 与未绑定同语义返回 null（键缺失）', () => {
    bindSession(ROOT, 's1', 'change-a');
    expect(lookupChange(ROOT, '')).toBeNull();
  });
});

// ===========================================================================
// bindSession — TTL 清理与自愈
// ===========================================================================

describe('bindSession — TTL 清理与自愈', () => {
  const TTL_MS = 24 * 60 * 60 * 1000;

  it('预置 updatedAt 超 24h 的旧条目后新 bind → 旧条目被清理、未过期条目保留', () => {
    const now = Date.now();
    const stale = new Date(now - TTL_MS - 60_000).toISOString();
    const fresh = new Date(now - 60_000).toISOString();
    writeRegistryRaw(
      ROOT,
      JSON.stringify({
        'stale-session': { change: 'old-change', updatedAt: stale },
        'fresh-session': { change: 'kept-change', updatedAt: fresh },
      }),
    );

    bindSession(ROOT, 'new-session', 'new-change');

    const registry = JSON.parse(readRegistryRaw(ROOT)!) as Record<
      string,
      { change: string; updatedAt: string }
    >;
    expect(Object.keys(registry).sort()).toEqual(['fresh-session', 'new-session']);
    expect(registry['fresh-session'].change).toBe('kept-change');
    expect(registry['new-session'].change).toBe('new-change');
    expect(lookupChange(ROOT, 'stale-session')).toBeNull();
  });

  it('TTL 阈值边界：恰好 24h 被清理（now - updated < TTL 为 false）；略小于 24h 保留', () => {
    const fixedNow = 1_800_000_000_000;
    const nowSpy = vi.spyOn(Date, 'now').mockReturnValue(fixedNow);
    try {
      // 恰好 24h：判定方向为「达到即过期」
      writeRegistryRaw(
        ROOT,
        JSON.stringify({
          'boundary-stale': {
            change: 'c1',
            updatedAt: new Date(fixedNow - TTL_MS).toISOString(),
          },
        }),
      );
      bindSession(ROOT, 'boundary-stale', 'c1-refreshed');
      // 该 session 被自身重绑覆盖，用第二个观察者验证清理行为
      expect(lookupChange(ROOT, 'boundary-stale')).toBe('c1-refreshed');

      // 略小于 24h 的其他条目被保留
      writeRegistryRaw(
        ROOT,
        JSON.stringify({
          keep: {
            change: 'kept-change',
            updatedAt: new Date(fixedNow - TTL_MS + 1000).toISOString(),
          },
        }),
      );
      bindSession(ROOT, 'new', 'new-change');
      expect(lookupChange(ROOT, 'keep')).toBe('kept-change');
      expect(lookupChange(ROOT, 'new')).toBe('new-change');
    } finally {
      nowSpy.mockRestore();
    }
  });

  it('损坏注册表（根非对象）后 bind → 自愈重写为合法结构', () => {
    writeRegistryRaw(ROOT, 'not-an-object{');

    bindSession(ROOT, 's1', 'change-a');

    const raw = readRegistryRaw(ROOT)!;
    const registry = JSON.parse(raw) as Record<string, unknown>;
    expect(registry).toBeTypeOf('object');
    expect(lookupChange(ROOT, 's1')).toBe('change-a');
  });

  it('bind 从不抛错：注册表目录不可写等异常被吞（fail-open）', () => {
    // 预置一个目录当作文件路径，writeFileSync 必然失败
    const filePath = expectedRegistryPath(ROOT);
    fs.mkdirSync(filePath, { recursive: true });

    expect(() => bindSession(ROOT, 's1', 'change-a')).not.toThrow();
    // 查询同样 fail-open
    expect(lookupChange(ROOT, 's1')).toBeNull();

    fs.rmdirSync(filePath);
  });
});
