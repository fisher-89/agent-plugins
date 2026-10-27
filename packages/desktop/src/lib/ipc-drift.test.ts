import { beforeEach, describe, expect, it, vi } from 'vite-plus/test';

import { type DriftEntry, driftCount, driftReports, reportDrift, resetDrift } from './ipc-drift';

// ---------------------------------------------------------------------------
// 纯内存聚合模块：零依赖、零 IO，故不引入任何 mock。用例隔离靠真实清空入口
// （`beforeEach` 调 `resetDrift()`），不靠 `vi.resetModules()`。
// ---------------------------------------------------------------------------

/** 一条上报的最小构造（默认值只为省样板，用例按需覆盖）。 */
function report(cmd: string, path: string, expected = 'string', got: unknown = 'x'): void {
  reportDrift({ cmd, path, expected, got });
}

/** 显式 `got` 的上报（不经默认参数，保证 `undefined` 原值可达）。 */
function reportGot(cmd: string, path: string, got: unknown): void {
  reportDrift({ cmd, path, expected: 'string', got });
}

/** 造 n 个互不相同的去重键（命令名带序号，路径固定根）。 */
function reportDistinct(count: number): void {
  for (let index = 0; index < count; index += 1) {
    report(`cmd-${index}`, '#');
  }
}

describe('reportDrift：按 (cmd, path) 去重累计', () => {
  beforeEach(() => {
    resetDrift();
  });

  it('首次上报一条完整报告 → 生成 1 条记录且四字段逐字一致、count 为 1', () => {
    reportDrift({
      cmd: 'listChanges',
      path: '#/0/addedAt',
      expected: 'number',
      got: 'not-a-number',
    });

    expect(driftCount()).toBe(1);
    const [entry] = driftReports();
    expect(entry).toEqual({
      cmd: 'listChanges',
      path: '#/0/addedAt',
      expected: 'number',
      got: 'not-a-number',
      count: 1,
    });
  });

  it.each([1, 2, 3])(
    '同一 (cmd, path) 连续上报 %i 次 → 仍 1 条记录且 count 等于上报次数',
    (times) => {
      for (let index = 0; index < times; index += 1) {
        report('addWorkspace', '#/addedAt');
      }

      expect(driftCount()).toBe(1);
      const entries = driftReports();
      expect(entries).toHaveLength(1);
      expect(entries[0].count).toBe(times);
    },
  );

  it('同 cmd 不同 path / 不同 cmd 同 path / 两者皆不同 → 三条独立条目互不合并', () => {
    report('codeStats', '#/totals/files');
    report('codeStats', '#/totals/code');
    report('dbRecords', '#/totals/files');
    report('dbRecords', '#/records/0/value');

    expect(driftCount()).toBe(4);
    expect(driftReports().map((entry) => [entry.cmd, entry.path])).toEqual([
      ['codeStats', '#/totals/files'],
      ['codeStats', '#/totals/code'],
      ['dbRecords', '#/totals/files'],
      ['dbRecords', '#/records/0/value'],
    ]);
    expect(driftReports().every((entry) => entry.count === 1)).toBe(true);
  });

  it('键分隔防混：cmd 含 "/" 与 path 分段错位的两组报告必须各占一条', () => {
    report('a/b', '#/c');
    report('a', '#/b/0/c');

    expect(driftCount()).toBe(2);
    expect(driftReports().map((entry) => `${entry.cmd}|${entry.path}`)).toEqual([
      'a/b|#/c',
      'a|#/b/0/c',
    ]);
  });

  it('path 取根 / 深层 / 超长（> 1000 字符）三形态均正常记入且互不冲突', () => {
    const longPath = `#/${'segment/'.repeat(200)}x`;
    expect(longPath.length).toBeGreaterThan(1000);

    report('listChanges', '#');
    report('getChangeDetail', '#/a/0/b');
    report('readArtifact', longPath);

    expect(driftCount()).toBe(3);
    const paths = driftReports().map((entry) => entry.path);
    expect(paths).toEqual(['#', '#/a/0/b', longPath]);
    expect(new Set(paths).size).toBe(3);
  });

  it.each([[undefined], [null], [Number.NaN]])('got 为 %s 时保留原值', (got) => {
    reportGot('saveAgentProvider', '#/models/high', got);

    const [entry] = driftReports();
    if (typeof got === 'number' && Number.isNaN(got)) {
      expect(Number.isNaN(entry.got)).toBe(true);
    } else {
      expect(entry.got).toBe(got);
    }
  });

  it('got 为超大对象引用 → 记录保留原引用（不深拷贝、不序列化）', () => {
    const huge = { deep: { nested: Array.from({ length: 10_000 }, (_, index) => index) } };

    reportGot('saveAgentProvider', '#/models/high', huge);

    const [entry] = driftReports();
    expect(entry.got).toBe(huge);
  });

  it('键数恰好达上限 200 仍接受，第 201 个新键被丢弃且既有键继续累计', () => {
    reportDistinct(200);
    expect(driftCount()).toBe(200);

    report('overflow-key', '#');
    expect(driftCount()).toBe(200);
    expect(driftReports().some((entry) => entry.cmd === 'overflow-key')).toBe(false);

    report('cmd-0', '#');
    const first = driftReports().find((entry) => entry.cmd === 'cmd-0');
    expect(first?.count).toBe(2);
    expect(driftCount()).toBe(200);
  });

  it('写入路径零副作用：调用前后 console 三面零调用、无未捕获异常', () => {
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const logSpy = vi.spyOn(console, 'log').mockImplementation(() => {});

    try {
      reportDrift({ cmd: 'listWorkspaces', path: '#', expected: 'array', got: 42 });

      expect(warnSpy).not.toHaveBeenCalled();
      expect(errorSpy).not.toHaveBeenCalled();
      expect(logSpy).not.toHaveBeenCalled();
      expect(driftCount()).toBe(1);
    } finally {
      warnSpy.mockRestore();
      errorSpy.mockRestore();
      logSpy.mockRestore();
    }
  });
});

describe('driftReports：聚合快照', () => {
  beforeEach(() => {
    resetDrift();
  });

  it('多条上报后按上报顺序给出，条目数等于去重键数且 count 与累计次数一致', () => {
    report('listChanges', '#/active');
    report('codeStats', '#/tree');
    report('listChanges', '#/archiveGroups');
    report('listChanges', '#/active');

    const entries = driftReports();
    expect(entries.map((entry) => entry.path)).toEqual(['#/active', '#/tree', '#/archiveGroups']);
    expect(entries.map((entry) => entry.count)).toEqual([2, 1, 1]);
    expect(entries).toHaveLength(driftCount());
  });

  it('空聚合 → 返回空数组（length 为 0，不是 undefined）', () => {
    const entries = driftReports();

    expect(Array.isArray(entries)).toBe(true);
    expect(entries.length).toBe(0);
  });

  it('快照语义：改动返回数组不改变后续读取到的聚合与计数', () => {
    report('listChanges', '#/active');

    const snapshot = driftReports() as DriftEntry[];
    snapshot.push({ cmd: 'x', path: '#', expected: 'string', got: 1, count: 9 });
    snapshot.length = 0;

    expect(driftReports()).toHaveLength(1);
    expect(driftCount()).toBe(1);
    expect(driftReports()[0].cmd).toBe('listChanges');
  });

  it('上限封顶后仍完整返回 200 条（无空洞、索引连续）', () => {
    reportDistinct(200);
    for (let index = 0; index < 20; index += 1) {
      report(`extra-${index}`, '#');
    }

    const entries = driftReports();
    expect(entries).toHaveLength(200);
    entries.forEach((entry, index) => {
      expect(entry.cmd).toBe(`cmd-${index}`);
      expect(entry.count).toBe(1);
    });
  });
});

describe('driftCount：去重键数（非上报次数）', () => {
  beforeEach(() => {
    resetDrift();
  });

  it('同键上报 5 次仍为 1，再加两处不同键 → 3', () => {
    for (let index = 0; index < 5; index += 1) {
      report('agentSessions', '#/total');
    }
    expect(driftCount()).toBe(1);

    report('agentSessions', '#/sessions');
    report('sessionDetail', '#/total');
    expect(driftCount()).toBe(3);
  });

  it('生命周期与容量组合：空聚合 → 0', () => {
    expect(driftCount()).toBe(0);
  });

  it('生命周期与容量组合：清空后 → 0', () => {
    report('listChanges', '#/active');
    resetDrift();

    expect(driftCount()).toBe(0);
  });

  it('生命周期与容量组合：封顶后继续上报新键仍恒为 200', () => {
    reportDistinct(200);
    report('cmd-300', '#');

    expect(driftCount()).toBe(200);
  });

  it('幂等读：连续两次调用返回值相同且不改变聚合', () => {
    report('listChanges', '#/active');
    report('listChanges', '#/archiveGroups');

    const first = driftCount();
    const second = driftCount();

    expect(first).toBe(2);
    expect(second).toBe(2);
    expect(driftReports()).toHaveLength(2);
  });
});

describe('resetDrift：清空聚合', () => {
  beforeEach(() => {
    resetDrift();
  });

  it('有数据时清空 → driftReports() 为 [] 且 driftCount() 为 0', () => {
    report('listChanges', '#/active');
    report('codeStats', '#/totals');

    resetDrift();

    expect(driftReports()).toEqual([]);
    expect(driftCount()).toBe(0);
  });

  it('已空时再次清空 → 不抛异常且保持空', () => {
    resetDrift();

    expect(() => {
      resetDrift();
    }).not.toThrow();
    expect(driftCount()).toBe(0);
  });

  it.each([
    ['listChanges', '#/active'],
    ['codeStats', '#/totals'],
  ])('清空后同键再次上报 → count 从 1 重新累计：%s %s', (cmd, path) => {
    report(cmd, path);
    report(cmd, path);
    expect(driftReports()[0].count).toBe(2);

    resetDrift();
    report(cmd, path);

    const entries = driftReports();
    expect(entries).toHaveLength(1);
    expect(entries[0].count).toBe(1);
    expect(entries[0].cmd).toBe(cmd);
  });

  it('清空零副作用：不触发 console、不抛', () => {
    report('listChanges', '#/active');
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    try {
      expect(() => {
        resetDrift();
      }).not.toThrow();
      expect(warnSpy).not.toHaveBeenCalled();
      expect(errorSpy).not.toHaveBeenCalled();
      expect(driftCount()).toBe(0);
    } finally {
      warnSpy.mockRestore();
      errorSpy.mockRestore();
    }
  });
});
