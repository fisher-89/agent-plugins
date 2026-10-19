import { describe, expect, it } from 'vite-plus/test';

import type { ExploreRecord } from '../types/dto';
import { groupExploreRecords } from './explore-groups';

/**
 * `groupExploreRecords` 纯函数（零依赖零 mock）：按 `promotedTo === null` 二分
 * 为 active / promoted 两组，组内先按 `createdAt` 倒序、同刻按 `id` 倒序兜底；
 * 入参数组顺序与元素字段调用前后零触碰（不 in-place sort），零 IO、零 invoke。
 */

const ROOT = 'C:\\demo\\alpha';
const BASE = 1727000000000;

/** 记录 fixture（三判别字段 `promotedTo` / `createdAt` / `id` 由用例显式覆盖）。 */
function makeRecord(overrides: Partial<ExploreRecord> = {}): ExploreRecord {
  return {
    id: 1,
    root: ROOT,
    name: 'api-retry',
    title: '接口重试策略',
    workflowType: 'requirement',
    promotedTo: null,
    sessionId: null,
    createdAt: BASE,
    updatedAt: BASE,
    ...overrides,
  };
}

/** 组内 `id` 序列（排序断言统一口径）。 */
function ids(group: ExploreRecord[]): number[] {
  return group.map((record) => record.id);
}

/** 组内排序不变量：`createdAt` 单调不增；同刻区间 `id` 严格递减。 */
function expectSorted(group: ExploreRecord[]): void {
  for (let index = 1; index < group.length; index += 1) {
    const prev = group[index - 1];
    const curr = group[index];
    expect(prev.createdAt).toBeGreaterThanOrEqual(curr.createdAt);
    if (prev.createdAt === curr.createdAt) {
      expect(prev.id).toBeGreaterThan(curr.id);
    }
  }
}

// ---------------------------------------------------------------------------
// 二分判据：promotedTo 为 null 与否
// ---------------------------------------------------------------------------

describe('groupExploreRecords：二分判据（promotedTo 为 null 与否，AC-1）', () => {
  it('正向：全进行中 fixture → 已转变更组为空数组、进行中组按 createdAt 倒序', () => {
    const records = [
      makeRecord({ id: 1, name: 'a', createdAt: BASE + 10 }),
      makeRecord({ id: 2, name: 'b', createdAt: BASE + 30 }),
      makeRecord({ id: 3, name: 'c', createdAt: BASE + 20 }),
    ];

    const { active, promoted } = groupExploreRecords(records);

    expect(promoted).toEqual([]);
    expect(promoted).toHaveLength(0);
    expect(ids(active)).toEqual([2, 3, 1]);
  });

  it('正向：全已转变更 fixture → 进行中组为空数组、已转变更组按 createdAt 倒序', () => {
    const records = [
      makeRecord({ id: 1, name: 'a', promotedTo: 'chg-1', createdAt: BASE + 20 }),
      makeRecord({ id: 2, name: 'b', promotedTo: 'chg-2', createdAt: BASE + 40 }),
      makeRecord({ id: 3, name: 'c', promotedTo: 'chg-3', createdAt: BASE + 30 }),
    ];

    const { active, promoted } = groupExploreRecords(records);

    expect(active).toEqual([]);
    expect(active).toHaveLength(0);
    expect(ids(promoted)).toEqual([2, 3, 1]);
  });

  it('正向：混合 fixture 两组并集 = 入参全集、交集为空（无丢漏无重复）', () => {
    const records = [
      makeRecord({ id: 1, name: 'active-a', createdAt: BASE + 50 }),
      makeRecord({ id: 2, name: 'active-b', createdAt: BASE + 20 }),
      makeRecord({ id: 3, name: 'active-c', createdAt: BASE + 40 }),
      makeRecord({ id: 4, name: 'promoted-a', promotedTo: 'chg-1', createdAt: BASE + 30 }),
      makeRecord({ id: 5, name: 'promoted-b', promotedTo: 'chg-2', createdAt: BASE + 10 }),
    ];

    const { active, promoted } = groupExploreRecords(records);

    expect(active.length + promoted.length).toBe(records.length);
    expect(active).toHaveLength(3);
    expect(promoted).toHaveLength(2);
    const activeIds = new Set(ids(active));
    const promotedIds = new Set(ids(promoted));
    expect([...activeIds].filter((id) => promotedIds.has(id))).toEqual([]);
    expect(new Set([...activeIds, ...promotedIds])).toEqual(new Set(ids(records)));
  });
});

// ---------------------------------------------------------------------------
// 组内排序：createdAt 倒序为主键、id 倒序为兜底键
// ---------------------------------------------------------------------------

describe('groupExploreRecords：组内排序（createdAt 倒序 + 同刻 id 倒序，AC-2）', () => {
  it('正向：createdAt 序与 id 序完全相反 → 按 createdAt 倒序（异于入参序与 id 降序）', () => {
    // 最新建档者 id 最小：id 1 = 最晚建档
    const records = [
      makeRecord({ id: 3, name: 'c', createdAt: BASE + 70 }),
      makeRecord({ id: 1, name: 'a', createdAt: BASE + 90 }),
      makeRecord({ id: 4, name: 'd', createdAt: BASE + 60 }),
      makeRecord({ id: 2, name: 'b', createdAt: BASE + 80 }),
    ];

    const { active } = groupExploreRecords(records);

    expect(ids(active)).toEqual([1, 2, 3, 4]);
    // 非入参原序
    expect(ids(active)).not.toEqual(ids(records));
    // 非 id 降序（后端返回顺序）
    expect(ids(active)).not.toEqual([4, 3, 2, 1]);
  });

  it('边界：组内 3 条 createdAt 全等、id 注入乱序 → 输出按 id 倒序', () => {
    const records = [
      makeRecord({ id: 5, name: 'a' }),
      makeRecord({ id: 2, name: 'b' }),
      makeRecord({ id: 9, name: 'c' }),
    ];

    const { active } = groupExploreRecords(records);

    expect(ids(active)).toEqual([9, 5, 2]);
    expectSorted(active);
  });

  it('边界：同刻与异刻交错 → 首键 createdAt 倒序优先、仅同刻区间按 id 倒序', () => {
    const records = [
      makeRecord({ id: 3, name: 'a-old', createdAt: BASE + 500 }),
      makeRecord({ id: 7, name: 'a-same-tick', createdAt: BASE + 500 }),
      makeRecord({ id: 5, name: 'a-latest', createdAt: BASE + 900 }),
      makeRecord({ id: 2, name: 'p-old', promotedTo: 'chg-1', createdAt: BASE + 700 }),
      makeRecord({ id: 8, name: 'p-same-tick', promotedTo: 'chg-1', createdAt: BASE + 700 }),
    ];

    const { active, promoted } = groupExploreRecords(records);

    // 900 单条在前（首键压倒 id 序），500 同刻对内 id 倒序
    expect(active.map((record) => record.name)).toEqual(['a-latest', 'a-same-tick', 'a-old']);
    expect(promoted.map((record) => record.name)).toEqual(['p-same-tick', 'p-old']);
  });

  it('边界：createdAt 取 0 / 负值 / MAX_SAFE_INTEGER → 按数值大小倒序（不做字符串比较）', () => {
    const records = [
      makeRecord({ id: 1, name: 'epoch', createdAt: 0, updatedAt: 0 }),
      makeRecord({ id: 2, name: 'pre-epoch', createdAt: -1000, updatedAt: -1000 }),
      makeRecord({
        id: 3,
        name: 'max',
        createdAt: Number.MAX_SAFE_INTEGER,
        updatedAt: Number.MAX_SAFE_INTEGER,
      }),
    ];

    const { active } = groupExploreRecords(records);

    expect(ids(active)).toEqual([3, 1, 2]);
    expect(active.map((record) => record.createdAt)).toEqual([Number.MAX_SAFE_INTEGER, 0, -1000]);
  });
});

// ---------------------------------------------------------------------------
// 空集与单元素边界
// ---------------------------------------------------------------------------

describe('groupExploreRecords：空集与单元素边界', () => {
  it('边界：空数组 → 两组均为空数组（在场且长度 0，非 undefined）', () => {
    const { active, promoted } = groupExploreRecords([]);

    expect(active).toEqual([]);
    expect(promoted).toEqual([]);
    expect(active).toHaveLength(0);
    expect(promoted).toHaveLength(0);
  });

  const singleCases: Array<[string, ExploreRecord, boolean]> = [
    ['单条进行中记录', makeRecord({ id: 1, name: 'solo-active' }), true],
    ['单条已转变更记录', makeRecord({ id: 2, name: 'solo-promoted', promotedTo: 'chg-1' }), false],
  ];

  it.each(singleCases)('边界：%s → 对应组长度 1、另一组长度 0', (_label, record, isActive) => {
    const { active, promoted } = groupExploreRecords([record]);

    expect(active).toHaveLength(isActive ? 1 : 0);
    expect(promoted).toHaveLength(isActive ? 0 : 1);
    expect((isActive ? active : promoted)[0]).toBe(record);
  });
});

// ---------------------------------------------------------------------------
// promotedTo 判据精确性：=== null（非 truthiness）
// ---------------------------------------------------------------------------

describe('groupExploreRecords：promotedTo 判据精确性（=== null 而非 truthiness）', () => {
  it('边界：promotedTo 空串 → 归已转变更组（空串为假值但仍非 null）', () => {
    const records = [
      makeRecord({ id: 1, name: 'active' }),
      makeRecord({ id: 2, name: 'empty-string', promotedTo: '' }),
      makeRecord({ id: 3, name: 'promoted', promotedTo: 'chg-1' }),
    ];

    const { active, promoted } = groupExploreRecords(records);

    expect(active.map((record) => record.name)).toEqual(['active']);
    expect(promoted).toHaveLength(2);
    expect(new Set(promoted.map((record) => record.name))).toEqual(
      new Set(['empty-string', 'promoted']),
    );
  });

  it('边界：元素 promotedTo 缺席（undefined，签名域外形态）→ 归已转变更组（不新增补缺分支）', () => {
    const missingField: ExploreRecord = {
      ...makeRecord({ id: 1, name: 'missing-field' }),
      promotedTo: undefined as unknown as string | null,
    };

    const { active, promoted } = groupExploreRecords([
      missingField,
      makeRecord({ id: 2, name: 'active' }),
    ]);

    expect(active.map((record) => record.name)).toEqual(['active']);
    expect(promoted.map((record) => record.name)).toEqual(['missing-field']);
  });

  it('边界：元素携带多余字段 → 分组排序只看三字段、返回元素为对象本体', () => {
    const extra = {
      ...makeRecord({ id: 4, name: 'extra', createdAt: BASE + 40 }),
      unknownField: '多余字段',
      anotherExtra: 42,
    };
    const plain = makeRecord({ id: 9, name: 'plain', createdAt: BASE + 90 });

    const { active } = groupExploreRecords([extra, plain]);

    expect(active.map((record) => record.name)).toEqual(['plain', 'extra']);
    expect(active[0]).toBe(plain);
    expect(active[1]).toBe(extra);
    expect(active[1]).toHaveProperty('unknownField', '多余字段');
  });
});

// ---------------------------------------------------------------------------
// 大列表与确定性
// ---------------------------------------------------------------------------

describe('groupExploreRecords：大列表与确定性', () => {
  it('边界：1000 条混合 fixture（createdAt 含重复值）→ 并集完备、组内单调不增、同刻 id 递减', () => {
    const records = Array.from({ length: 1000 }, (_, index) => {
      const id = index + 1;
      const createdAt = BASE + ((index * 7) % 50);
      return makeRecord({
        id,
        name: `topic-${id}`,
        title: `主题 ${id}`,
        promotedTo: index % 3 === 0 ? null : `chg-${id}`,
        createdAt,
        updatedAt: createdAt,
      });
    });

    const { active, promoted } = groupExploreRecords(records);

    expect(active.length + promoted.length).toBe(records.length);
    expect(active).toHaveLength(records.filter((record) => record.promotedTo === null).length);
    expect(new Set([...ids(active), ...ids(promoted)])).toEqual(new Set(ids(records)));
    expectSorted(active);
    expectSorted(promoted);
  });

  it('边界：同一入参连续调用两次 → 两组 id 序列逐元素相同（无时间源 / 随机 / IO）', () => {
    const records = [
      makeRecord({ id: 3, name: 'c', createdAt: BASE + 30 }),
      makeRecord({ id: 1, name: 'a', promotedTo: 'chg-1', createdAt: BASE + 10 }),
      makeRecord({ id: 2, name: 'b', createdAt: BASE + 20 }),
      makeRecord({ id: 5, name: 'e', promotedTo: 'chg-2', createdAt: BASE + 50 }),
      makeRecord({ id: 4, name: 'd', createdAt: BASE + 20 }),
    ];

    const first = groupExploreRecords(records);
    const second = groupExploreRecords(records);

    expect(ids(second.active)).toEqual(ids(first.active));
    expect(ids(second.promoted)).toEqual(ids(first.promoted));
  });

  it('边界：返回数组与入参数组零共享（push / 重排后入参长度与顺序不变）', () => {
    const records = [
      makeRecord({ id: 1, name: 'a', createdAt: BASE + 10 }),
      makeRecord({ id: 2, name: 'b', createdAt: BASE + 20 }),
    ];

    const { active, promoted } = groupExploreRecords(records);

    expect(active).not.toBe(records);
    expect(promoted).not.toBe(records);
    active.push(makeRecord({ id: 77, name: 'injected' }));
    expect(records).toHaveLength(2);
    active.reverse();
    expect(ids(records)).toEqual([1, 2]);
  });
});

// ---------------------------------------------------------------------------
// 无副作用与域外入参
// ---------------------------------------------------------------------------

describe('groupExploreRecords：入参只读与域外入参（AC-3）', () => {
  it('异常：入参数组顺序与元素全字段调用前后全等（不 in-place sort）', () => {
    const records = [
      makeRecord({ id: 3, name: 'c', title: '标题 c', createdAt: BASE + 30 }),
      makeRecord({ id: 1, name: 'a', title: '标题 a', promotedTo: 'chg-1', createdAt: BASE + 10 }),
      makeRecord({ id: 2, name: 'b', title: '标题 b', createdAt: BASE + 20 }),
    ];
    const snapshot = records.map((record) => ({ ...record }));

    groupExploreRecords(records);

    expect(records).toEqual(snapshot);
    expect(records.map((record) => record.name)).toEqual(['c', 'a', 'b']);
    expect(ids(records)).toEqual([3, 1, 2]);
  });

  it('异常：records 为 undefined（类型断言越过签名）→ 抛 TypeError 而非静默返回空分组', () => {
    expect(() => groupExploreRecords(undefined as unknown as ExploreRecord[])).toThrow(TypeError);
    expect(() => groupExploreRecords(null as unknown as ExploreRecord[])).toThrow(TypeError);
  });
});
