import type { Channel } from '@tauri-apps/api/core';
import { afterEach, beforeEach, describe, expect, expectTypeOf, it, vi } from 'vite-plus/test';
import { ZodObject, ZodType, type z } from 'zod';

import {
  type AgentEngineKind,
  type AgentEvent,
  type AgentPermissionMode,
  type AgentProviderProtocol,
  type AgentRunMessage,
  type ChangeList,
  type CodeStatsReport,
  type DirNode,
  type ExecOutputChunk,
  type FileSlice,
  type FileWatchEvent,
  type RunNotice,
  type SessionSummary,
  type TreeEntry,
  type TurnSummary,
  commands,
} from '../types/generated/bindings';
import {
  type DirNodeSchema,
  TreeEntrySchema,
  argsSchemas,
  channelArgs,
  resultSchemas,
} from '../types/generated/bindings.zod';
import { ipc } from './ipc';
import { driftCount, resetDrift } from './ipc-drift';

// 进程边界替身（只替 Tauri IPC）：`bindings.ts` 在模块装载时就解构
// `invoke` / `Channel`，故两个名字都必须导出。soft-report 分级链路的断言需要
// 真实 `ipc` 代理走一遍 `invoke` 往返——替身化 `ipc.ts` / `ipc-drift.ts` 会让
// 「零漂移 / 恰 1 条」自证。
const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
  Channel: class {
    public onmessage: ((message: unknown) => void) | null = null;
  },
}));

// ---------------------------------------------------------------------------
// 契约等价面：生成物三表（`bindings.zod.ts`）与类型唯一源（`bindings.ts`）的
// 逐命令等价断言。全部**真实 import**（不 mock zod、不 mock 生成物）——运行期
// zod 单实例是 `.meta()` / `instanceof` 可观测的前提，替身化会让等价断言失去
// 判据效力。
//
// 类型断言由 `vp check` 的 typeCheck 执行；运行期断言由 `vp test` 执行。等价
// 断言的另一端类型只从 `bindings.ts` 取（类型主权不转移）。
// ---------------------------------------------------------------------------

const commandKeys = Object.keys(commands) as Array<keyof typeof commands>;

/** 有 `Channel` 形参的命令（信封面表项应恰为 `onEvent`）。 */
const channelCommands = [
  'agentStart',
  'agentSessionWatch',
  'changeFlowStart',
  'changeFlowWatch',
  'watchSubscribe',
] as const;

/** 互相可赋值面等价判据：`zod` 的 `.and()` 推面会把交叉联合展平（成员补可选位），
 *  与 `bindings.ts` 的 `A & (B | C)` 表示不构成内部同构，但仍应互相可赋值。 */
type MutualAssignable<A, B> = [A] extends [B] ? ([B] extends [A] ? true : false) : false;

/** 逐命令结果面不等价的命令名集合（断言为 `never` 即全量等价）。 */
type ResultMismatchKeys = {
  [K in keyof typeof commands]: MutualAssignable<
    z.infer<(typeof resultSchemas)[K]>,
    Awaited<ReturnType<(typeof commands)[K]>>
  > extends true
    ? never
    : K;
}[keyof typeof commands];

/** 参数面推导是否落在对象类型（不塌成 `unknown` / `any`）的不合格命令名集合。 */
type ArgsKeyMismatchKeys = {
  [K in keyof typeof commands]: z.infer<(typeof argsSchemas)[K]> extends Record<string, unknown>
    ? never
    : K;
}[keyof typeof commands];

/** 逐命令形参元组是否与 `ipc` 侧同名命令等价的不合格命令名集合（断言为 `never` 即全量等价）。 */
type ParamsMismatchKeys = {
  [K in keyof typeof commands]: MutualAssignable<
    Parameters<(typeof commands)[K]>,
    Parameters<(typeof ipc)[K]>
  > extends true
    ? never
    : K;
}[keyof typeof commands];

/** `readArtifact` 结果为 `ArtifactEnvelope | null`，取非空臂做字段面断言。 */
type ArtifactEnvelopeView = NonNullable<z.infer<typeof resultSchemas.readArtifact>>;

/** 取形参元组首位的 `Channel<T>` 载荷类型。 */
type ChannelPayload<T> = T extends Channel<infer U> ? U : never;

// ---------------------------------------------------------------------------
// 三表 runtime 面
// ---------------------------------------------------------------------------

describe('三表 runtime 面：键集与 commands 逐一对应', () => {
  it('argsSchemas / resultSchemas / channelArgs 的键集与 commands 完全一致', () => {
    expect(Object.keys(argsSchemas)).toEqual(Object.keys(commands));
    expect(Object.keys(resultSchemas)).toEqual(Object.keys(commands));
    expect(Object.keys(channelArgs)).toEqual(Object.keys(commands));
  });

  it.each(commandKeys)('%s 的三表表项均为 getter（零初始化成本、无 TDZ）', (key) => {
    for (const table of [argsSchemas, resultSchemas, channelArgs]) {
      const descriptor = Object.getOwnPropertyDescriptor(table, key);
      expect(descriptor).toBeDefined();
      expect(typeof descriptor?.get).toBe('function');
    }
  });

  // 名称无关面（类型层取不到形参名，design R-4）：键数 == `commands[k].length`。
  // 逐命令的**名字**面判据在 `ipc.test.ts` 的「payload 键集 == argsSchemas.shape
  // 键集」臂（`invoke` 第二实参键集是唯一逐命令名称源）。
  it.each(commandKeys)('%s 的参数表键数与 commands 的形参个数一致', (key) => {
    const schema = Reflect.get(argsSchemas, key) as ZodObject;
    expect(schema).toBeInstanceOf(ZodObject);
    expect(Object.keys(schema.shape)).toHaveLength(commands[key].length);
  });

  // 名称形态面（逐命令）：lowerCamelCase、无重名，且信封面键名必落在参数面键集内
  // （`channelArgs` 是 `argsSchemas` 的形参名子集——两表独立生成，名字必须对得上）。
  it.each(commandKeys)('%s 的形参名为唯一 lowerCamelCase 且信封面键名在参数面键集内', (key) => {
    const schema = Reflect.get(argsSchemas, key) as ZodObject;
    const shapeKeys = Object.keys(schema.shape);
    expect(new Set(shapeKeys).size).toBe(shapeKeys.length);
    for (const name of shapeKeys) expect(name).toMatch(/^[a-z][A-Za-z0-9]*$/);
    for (const name of Object.keys(Reflect.get(channelArgs, key) as object)) {
      expect(shapeKeys).toContain(name);
    }
  });

  it.each([['listWorkspaces'], ['appEnvironment']])(
    '零形参命令 %s 推导为空 strictObject（strictObject({}) 形态）',
    (key) => {
      const schema = Reflect.get(argsSchemas, key) as ZodObject;
      expect(Object.keys(schema.shape)).toEqual([]);
      expect(schema.safeParse({}).success).toBe(true);
      expect(schema.safeParse(undefined).success).toBe(false);
    },
  );

  it('参数键名为 lowerCamelCase 且键序为 Rust 形参序（since_seq → sinceSeq）', () => {
    expect(Object.keys((argsSchemas.agentSessionWatch as ZodObject).shape)).toEqual([
      'onEvent',
      'root',
      'sessionId',
      'sinceSeq',
    ]);
    expect(Object.keys((argsSchemas.saveAgentProvider as ZodObject).shape)).toEqual([
      'id',
      'name',
      'protocol',
      'baseUrl',
      'apiKey',
      'models',
      'contextLength',
      'maxTokens',
      'webSearch',
    ]);
  });

  it('降级哨兵的 meta 可回读（运行期 zod 单实例 + 4.x `.meta()` 面）', () => {
    const sentinel = (argsSchemas.agentStart as ZodObject).shape.onEvent;
    expect(sentinel).toBeInstanceOf(ZodType);
    expect((sentinel as ZodType).meta()).toMatchObject({
      degraded: expect.stringContaining('channel'),
    });
  });

  it('信封面：5 个 Channel 命令表项恰为 onEvent 且值为 ZodType', () => {
    for (const key of channelCommands) {
      const table = Reflect.get(channelArgs, key) as Record<string, unknown>;
      expect(Object.keys(table)).toEqual(['onEvent']);
      expect(table.onEvent).toBeInstanceOf(ZodType);
    }
  });

  it('非 Channel 命令在 channelArgs 中无键（表只含 Channel 形参）', () => {
    const withChannel = commandKeys.filter(
      (key) => Object.keys(Reflect.get(channelArgs, key) as object).length > 0,
    );
    expect([...withChannel].sort()).toEqual([...channelCommands].sort());
    expect(Object.keys(Reflect.get(channelArgs, 'listChanges') as object)).toEqual([]);
    expect(Object.keys(Reflect.get(channelArgs, 'dbRecords') as object)).toEqual([]);
  });
});

// ---------------------------------------------------------------------------
// 命令调用面：`ipc` 的形参元组逐命令与 `commands` 等价（`commands.` → `ipc.` 机械
// 替换零形态漂移）。类型层的形参**名**不可得（design R-4：`Parameters<…>` 只给位置
// 元组），故名称面的逐命令判据落在 `ipc.test.ts` 的「payload 键集 == shape 键集」臂
// 与本节下方的「键数 == 形参个数」臂。
// ---------------------------------------------------------------------------

describe('命令调用面：ipc 与 commands 的形参元组逐命令 Equal', () => {
  it('逐命令 Parameters<commands[K]> 与 Parameters<ipc[K]> Equal（全命令键）', () => {
    expectTypeOf<ParamsMismatchKeys>().toEqualTypeOf<never>();
  });

  it('反例校准：ipc 侧形参元组不等于放宽的 unknown[] 形态（Equal 恒真性被排除）', () => {
    expectTypeOf<Parameters<typeof ipc.listChanges>>().not.toEqualTypeOf<unknown[]>();
    expectTypeOf<Parameters<typeof ipc.watchSubscribe>>().not.toEqualTypeOf<unknown[]>();
  });
});

// ---------------------------------------------------------------------------
// resultSchemas 类型面（逐命令全量 Equal）
// ---------------------------------------------------------------------------

describe('resultSchemas 类型面：与 bindings.ts 逐命令等价', () => {
  it('逐命令 z.infer 与 Awaited<ReturnType<typeof commands[K]>> 全量 Equal', () => {
    expectTypeOf<ResultMismatchKeys>().toEqualTypeOf<never>();
  });

  it('逐命令参数面推导为对象类型（不塌成 unknown / any）', () => {
    expectTypeOf<ArgsKeyMismatchKeys>().toEqualTypeOf<never>();
  });

  it('Option 面 ⇒ 含 | null（sessionDetail 的「未找到」）', () => {
    expectTypeOf<
      z.infer<typeof resultSchemas.sessionDetail>
    >().toEqualTypeOf<SessionSummary | null>();
  });

  it('f64 面 ⇒ number（非 bigint / 非 string）', () => {
    expectTypeOf<z.infer<typeof resultSchemas.codeStats>>().toEqualTypeOf<CodeStatsReport>();
    expectTypeOf<
      z.infer<typeof resultSchemas.codeStats>['totals']['files']
    >().toEqualTypeOf<number>();
  });

  it('整数面（i64 / u64 映射位）⇒ number，非 bigint', () => {
    expectTypeOf<
      z.infer<typeof argsSchemas.agentSessionWatch>['sinceSeq']
    >().toEqualTypeOf<number>();
    expectTypeOf<z.infer<typeof argsSchemas.agentSessions>['offset']>().toEqualTypeOf<number>();
    expectTypeOf<ArtifactEnvelopeView['version']>().toEqualTypeOf<number>();
  });

  it('字符串面 ⇒ string（ChangeList 具名 DTO 逐字段对齐）', () => {
    expectTypeOf<z.infer<typeof resultSchemas.listChanges>>().toEqualTypeOf<ChangeList>();
  });

  it('literal 枚举面 AgentEngineKind 与具名联合类型 Equal', () => {
    expectTypeOf<
      z.infer<typeof resultSchemas.saveAgentInstance>['engine']
    >().toEqualTypeOf<AgentEngineKind>();
  });

  it('literal 枚举面 AgentPermissionMode 与具名联合类型 Equal', () => {
    expectTypeOf<
      z.infer<typeof argsSchemas.agentStart>['permissionMode']
    >().toEqualTypeOf<AgentPermissionMode>();
  });

  it('literal 枚举面 AgentProviderProtocol 与具名联合类型 Equal', () => {
    expectTypeOf<
      z.infer<typeof argsSchemas.saveAgentProvider>['protocol']
    >().toEqualTypeOf<AgentProviderProtocol>();
  });

  it('Intersection 面（AgentEvent 的 flatten 变体联合）互相可赋值', () => {
    expectTypeOf<
      MutualAssignable<z.infer<typeof resultSchemas.agentSessionTranscript>, AgentEvent[]>
    >().toEqualTypeOf<true>();
    expectTypeOf<z.infer<typeof resultSchemas.agentSessionTranscript>>().not.toBeAny();
  });

  it('unknown 面（ArtifactEnvelope.payload 等 serde_json::Value 出线字段）为 unknown 且非 any', () => {
    expectTypeOf<ArtifactEnvelopeView['payload']>().toEqualTypeOf<unknown>();
    expectTypeOf<ArtifactEnvelopeView['payload']>().not.toBeAny();
    expectTypeOf<
      z.infer<typeof resultSchemas.dbRecords>['records'][0]['value']
    >().toEqualTypeOf<unknown>();
  });

  it('agentStart 结果面与 TurnSummary Equal（record 信封承载同一类型）', () => {
    expectTypeOf<z.infer<typeof resultSchemas.agentStart>>().toEqualTypeOf<TurnSummary>();
  });

  it('反例校准：去掉 nullable 的窄类型必须不等价（Equal 恒真性被排除）', () => {
    expectTypeOf<z.infer<typeof resultSchemas.sessionDetail>>().not.toEqualTypeOf<SessionSummary>();
    expectTypeOf<z.infer<typeof argsSchemas.agentStart>['sessionId']>().not.toEqualTypeOf<string>();
  });
});

// ---------------------------------------------------------------------------
// argsSchemas 类型面
// ---------------------------------------------------------------------------

describe('argsSchemas 类型面：必填键 + null 可过（未用 .optional() 放宽）', () => {
  it('Option<number> 面 ⇒ number | null 且键必填（非 Partial）', () => {
    expectTypeOf<z.infer<typeof argsSchemas.saveAgentProvider>['id']>().toEqualTypeOf<
      number | null
    >();
    expectTypeOf<z.infer<typeof argsSchemas.saveAgentProvider>>().not.toEqualTypeOf<
      Partial<z.infer<typeof argsSchemas.saveAgentProvider>>
    >();
  });

  it('Option<string> 面 ⇒ string | null（createExploreRecord 的第二形参）', () => {
    expectTypeOf<z.infer<typeof argsSchemas.createExploreRecord>['name']>().toEqualTypeOf<
      string | null
    >();
  });

  it('嵌套 strict 面在运行期拒绝多余子键（models 深层）', () => {
    const base = {
      id: null,
      name: 'prov',
      protocol: 'openai',
      baseUrl: 'https://api.example.com',
      apiKey: 'sk-1',
      contextLength: null,
      maxTokens: null,
      webSearch: false,
    };

    const valid = argsSchemas.saveAgentProvider.safeParse({
      ...base,
      models: { high: 'h', medium: 'm', low: 'l' },
    });
    expect(valid.success).toBe(true);

    const surplus = argsSchemas.saveAgentProvider.safeParse({
      ...base,
      models: { high: 'h', medium: 'm', low: 'l', surplus: 'x' },
    });
    expect(surplus.success).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// channelArgs 类型面
// ---------------------------------------------------------------------------

describe('channelArgs 类型面：信封 T 与 bindings.ts 首参 Channel<T> 一致', () => {
  it.each([['agentStart'], ['agentSessionWatch']] as const)(
    '%s 的信封 schema 与首参 Channel<T> 的 T 等价（AgentRunMessage）',
    (key) => {
      expectTypeOf<
        MutualAssignable<z.infer<(typeof channelArgs)[typeof key]['onEvent']>, AgentRunMessage>
      >().toEqualTypeOf<true>();
      expectTypeOf<
        MutualAssignable<
          z.infer<(typeof channelArgs)[typeof key]['onEvent']>,
          ChannelPayload<Parameters<(typeof commands)[typeof key]>[0]>
        >
      >().toEqualTypeOf<true>();
    },
  );

  it.each([['changeFlowStart'], ['changeFlowWatch']] as const)(
    '%s 的信封 schema 与首参 Channel<T> 的 T Equal（RunNotice）',
    (key) => {
      expectTypeOf<
        z.infer<(typeof channelArgs)[typeof key]['onEvent']>
      >().toEqualTypeOf<RunNotice>();
      expectTypeOf<z.infer<(typeof channelArgs)[typeof key]['onEvent']>>().toEqualTypeOf<
        ChannelPayload<Parameters<(typeof commands)[typeof key]>[0]>
      >();
    },
  );

  it('watchSubscribe 的信封 schema 与首参 Channel<T> 的 T Equal（FileWatchEvent）', () => {
    expectTypeOf<
      z.infer<typeof channelArgs.watchSubscribe.onEvent>
    >().toEqualTypeOf<FileWatchEvent>();
    expectTypeOf<z.infer<typeof channelArgs.watchSubscribe.onEvent>>().toEqualTypeOf<
      ChannelPayload<Parameters<typeof commands.watchSubscribe>[0]>
    >();
  });

  it('反例校准：信封 schema 不等于整个 Channel 实例类型（只描述载荷）', () => {
    expectTypeOf<z.infer<typeof channelArgs.agentStart.onEvent>>().not.toEqualTypeOf<
      Channel<AgentRunMessage>
    >();
  });
});

// ---------------------------------------------------------------------------
// 具名类型 schema 段：递归与降级面
// ---------------------------------------------------------------------------

describe('具名类型 schema 段：递归与降级面', () => {
  it('递归类型 TreeEntry ↔ DirNode 不塌 any（getter 化后自引用可解）', () => {
    expectTypeOf<z.infer<typeof TreeEntrySchema>>().toEqualTypeOf<TreeEntry>();
    expectTypeOf<z.infer<typeof TreeEntrySchema>>().not.toBeAny();
    expectTypeOf<z.infer<typeof DirNodeSchema>>().toEqualTypeOf<DirNode>();
    expectTypeOf<z.infer<typeof DirNodeSchema>>().not.toBeAny();
  });

  it('降级面哨兵 z.infer 为 unknown（不冒充具体类型）', () => {
    expectTypeOf<z.infer<typeof argsSchemas.watchSubscribe>['onEvent']>().toEqualTypeOf<unknown>();
    expectTypeOf<z.infer<typeof argsSchemas.changeFlowStart>['onEvent']>().toEqualTypeOf<unknown>();
    expectTypeOf<z.infer<typeof argsSchemas.changeFlowWatch>['onEvent']>().toEqualTypeOf<unknown>();
  });

  it('递归 schema 在运行期可解析嵌套树（自引用初值不抛 TDZ）', () => {
    const parsed = TreeEntrySchema.safeParse({
      kind: 'dir',
      node: {
        name: 'src',
        path: '/src',
        files: 1,
        code: 10,
        comments: 2,
        blanks: 3,
        children: [
          {
            kind: 'file',
            node: { name: 'a.ts', path: '/src/a.ts', code: 10, comments: 2, blanks: 3 },
          },
        ],
      },
    });

    expect(parsed.success).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// 三新命令（change-detail-live-stdout / AC-14）：`execOutputPath` /
// `readExecCommand` / `readExecOutputDelta` 与出线 DTO `ExecOutputChunk`。
//
// 等价断言两端都在真实 import 上：类型主权端是 `bindings.ts`（`commands` 与
// `ExecOutputChunk`），schema 端是 `bindings.zod.ts`（`argsSchemas` /
// `resultSchemas`）。运行期「脏结果只上报」行经真实 `ipc` 代理（只替 `invoke`
// 进程边界），soft-report 分级与漂移聚合均为真实实现。
// ---------------------------------------------------------------------------

/** 三新命令键（登记序与 `all_commands!` 同序）。 */
const execCommandKeys = ['execOutputPath', 'readExecCommand', 'readExecOutputDelta'] as const;

/** `readExecOutputDelta` 的出线面：类型端非空臂。 */
type DeltaChunk = NonNullable<Awaited<ReturnType<typeof commands.readExecOutputDelta>>>;

/** `readExecOutputDelta` 的出线面：schema 端非空臂。 */
type DeltaSchemaChunk = NonNullable<z.infer<typeof resultSchemas.readExecOutputDelta>>;

// DEV 汇总告警的观测面静音（告警本身由 `ipc.test.ts` 断言；本节只断漂移计数）
let warnSpy: ReturnType<typeof vi.spyOn>;

beforeEach(() => {
  invokeMock.mockReset();
  resetDrift();
  warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});
});

afterEach(() => {
  warnSpy.mockRestore();
});

describe('三新命令：键集与形参元组', () => {
  it('commands 含三新命令且各恰 1 次（无重复登记）', () => {
    for (const key of execCommandKeys) {
      expect(commandKeys.filter((candidate) => candidate === key)).toHaveLength(1);
    }
  });

  it.each([
    ['execOutputPath', 3],
    ['readExecCommand', 3],
    ['readExecOutputDelta', 4],
  ] as const)('%s 的形参个数与 argsSchemas 键数同为 %i', (key, arity) => {
    expect(commands[key]).toHaveLength(arity);
    expect(Object.keys((argsSchemas[key] as ZodObject).shape)).toHaveLength(arity);
  });

  it('三新命令的形参元组与三/四元字符串元组互相可赋值', () => {
    expectTypeOf<
      MutualAssignable<Parameters<typeof commands.execOutputPath>, [string, string, string]>
    >().toEqualTypeOf<true>();
    expectTypeOf<
      MutualAssignable<Parameters<typeof commands.readExecCommand>, [string, string, string]>
    >().toEqualTypeOf<true>();
    expectTypeOf<
      MutualAssignable<
        Parameters<typeof commands.readExecOutputDelta>,
        [string, string, string, number]
      >
    >().toEqualTypeOf<true>();
    // 反例校准：末位 `fromOffset` 不是字符串（四参形态非「四个 string」）
    expectTypeOf<
      MutualAssignable<
        Parameters<typeof commands.readExecOutputDelta>,
        [string, string, string, string]
      >
    >().toEqualTypeOf<false>();
  });

  it('三新命令的 argsSchemas 形参名逐字为 lowerCamelCase（含 fromOffset）', () => {
    expect(Object.keys((argsSchemas.execOutputPath as ZodObject).shape)).toEqual([
      'root',
      'changeName',
      'execId',
    ]);
    expect(Object.keys((argsSchemas.readExecCommand as ZodObject).shape)).toEqual([
      'root',
      'changeName',
      'execId',
    ]);
    expect(Object.keys((argsSchemas.readExecOutputDelta as ZodObject).shape)).toEqual([
      'root',
      'changeName',
      'execId',
      'fromOffset',
    ]);
  });
});

describe('ExecOutputChunk 出线形态：MutualAssignable 正反例与 nullable', () => {
  it('类型端与 schema 端互相可赋值（两端都是 { text: string; nextOffset: number }）', () => {
    expectTypeOf<MutualAssignable<DeltaChunk, DeltaSchemaChunk>>().toEqualTypeOf<true>();
    expectTypeOf<MutualAssignable<DeltaChunk, ExecOutputChunk>>().toEqualTypeOf<true>();
    expectTypeOf<MutualAssignable<DeltaSchemaChunk, ExecOutputChunk>>().toEqualTypeOf<true>();
    expectTypeOf<DeltaChunk>().not.toBeAny();
  });

  it('反例校准：异型字段形态不互相可赋值（等价断言非恒真、any 不静默通过）', () => {
    expectTypeOf<
      MutualAssignable<DeltaChunk, { text: number; nextOffset: number }>
    >().toEqualTypeOf<false>();
    expectTypeOf<
      MutualAssignable<DeltaChunk, { text: string; nextOffset: string }>
    >().toEqualTypeOf<false>();
    expectTypeOf<MutualAssignable<DeltaChunk, { text: string }>>().toEqualTypeOf<false>();
    expectTypeOf<MutualAssignable<DeltaChunk, string>>().toEqualTypeOf<false>();
  });

  it('Option 出线为 nullable：三新命令返回类型均含 | null', () => {
    expectTypeOf<Awaited<ReturnType<typeof commands.execOutputPath>>>().toEqualTypeOf<
      string | null
    >();
    expectTypeOf<Awaited<ReturnType<typeof commands.readExecCommand>>>().toEqualTypeOf<
      string | null
    >();
    expectTypeOf<
      Awaited<ReturnType<typeof commands.readExecOutputDelta>>
    >().toEqualTypeOf<ExecOutputChunk | null>();
    // 反例校准：丢掉 null 臂即不等价（nullable 未被压成非空类型）
    expectTypeOf<
      Awaited<ReturnType<typeof commands.readExecCommand>>
    >().not.toEqualTypeOf<string>();
    expectTypeOf<
      Awaited<ReturnType<typeof commands.readExecOutputDelta>>
    >().not.toEqualTypeOf<ExecOutputChunk>();
  });

  it('nextOffset 为 number（非 bigint）：类型面持衡、schema 面对 bigint 拒绝', () => {
    expectTypeOf<ExecOutputChunk['nextOffset']>().toEqualTypeOf<number>();
    expectTypeOf<DeltaSchemaChunk['nextOffset']>().toEqualTypeOf<number>();
    expectTypeOf<MutualAssignable<ExecOutputChunk['nextOffset'], bigint>>().toEqualTypeOf<false>();
    expect(
      (resultSchemas.readExecOutputDelta as ZodType).safeParse({ text: 'a', nextOffset: 7n })
        .success,
    ).toBe(false);
  });

  it('空面类型零窄化：text 接受空串、nextOffset 接受 0（空记录形态可表达）', () => {
    const emptyRun: ExecOutputChunk = { text: '', nextOffset: 0 };
    expect(emptyRun.text).toBe('');
    expect(emptyRun.nextOffset).toBe(0);
    expect(
      (resultSchemas.readExecOutputDelta as ZodType).safeParse({ text: '', nextOffset: 0 }).success,
    ).toBe(true);
  });

  it('miss 形态：schema 接受 null（Option 出线 nullable），非对象形态被拒', () => {
    const schema = resultSchemas.readExecOutputDelta as ZodType;
    expect(schema.safeParse(null).success).toBe(true);
    expect(schema.safeParse({ text: '', nextOffset: 0 }).success).toBe(true);
    expect(schema.safeParse(undefined).success).toBe(false);
    expect(schema.safeParse('text').success).toBe(false);
  });
});

describe('fromOffset 入线：必填 number（零 optional 放宽）', () => {
  const base = { root: '/ws', changeName: 'add-feature', execId: 'exec-1' };

  it('类型面：fromOffset 为必填 number（非 optional / 非 string）', () => {
    expectTypeOf<
      z.infer<typeof argsSchemas.readExecOutputDelta>['fromOffset']
    >().toEqualTypeOf<number>();
    expectTypeOf<z.infer<typeof argsSchemas.readExecOutputDelta>>().not.toEqualTypeOf<
      Partial<z.infer<typeof argsSchemas.readExecOutputDelta>>
    >();
    expectTypeOf<
      MutualAssignable<
        z.infer<typeof argsSchemas.readExecOutputDelta>['fromOffset'],
        number | undefined
      >
    >().toEqualTypeOf<false>();
  });

  it.each([
    ['四参齐全（0）', { ...base, fromOffset: 0 }, true],
    ['四参齐全（极大值）', { ...base, fromOffset: Number.MAX_SAFE_INTEGER }, true],
    ['缺 fromOffset', base, false],
    ['fromOffset 为字符串', { ...base, fromOffset: '0' }, false],
    ['多余键', { ...base, fromOffset: 0, surplus: 1 }, false],
  ] as const)('运行期 schema 面：%s → success=%s', (_label, payload, expected) => {
    expect((argsSchemas.readExecOutputDelta as ZodType).safeParse(payload).success).toBe(expected);
  });
});

describe('三新命令结果面：脏结果只 soft-report 恰 1 条漂移', () => {
  const args = ['/ws', 'add-feature', 'exec-1', 0] as const;

  it('合法结果 → 零漂移、原引用透传', async () => {
    const clean: ExecOutputChunk = { text: 'ok\n', nextOffset: 3 };
    invokeMock.mockResolvedValue(clean);

    const result = await ipc.readExecOutputDelta(...args);

    expect(result).toBe(clean);
    expect(driftCount()).toBe(0);
  });

  it('null（miss）→ 零漂移（Option 出线 nullable 合法形态）', async () => {
    invokeMock.mockResolvedValue(null);

    const result = await ipc.readExecOutputDelta(...args);

    expect(result).toBeNull();
    expect(driftCount()).toBe(0);
  });

  it.each([
    ['text 给数字', { text: 42, nextOffset: 7 }],
    ['缺 nextOffset', { text: 'a' }],
  ] as const)('脏结果（%s）→ 恰 1 条漂移、原值透传不 throw', async (_label, dirty) => {
    invokeMock.mockResolvedValue(dirty);

    const result = await ipc.readExecOutputDelta(...args);

    expect(result).toBe(dirty);
    expect(driftCount()).toBe(1);
  });

  it('同一命令两次脏结果 → 仍 1 条去重键（count 累计不膨胀）', async () => {
    invokeMock.mockResolvedValue({ text: 42, nextOffset: 7 });

    await ipc.readExecOutputDelta(...args);
    await ipc.readExecOutputDelta(...args);

    expect(driftCount()).toBe(1);
  });

  it('execOutputPath / readExecCommand 的脏结果同样恰 1 条漂移', async () => {
    invokeMock.mockResolvedValue(42);
    expect(await ipc.execOutputPath('/ws', 'add-feature', 'exec-1')).toBe(42);
    expect(driftCount()).toBe(1);

    resetDrift();
    invokeMock.mockResolvedValue(42);
    expect(await ipc.readExecCommand('/ws', 'add-feature', 'exec-1')).toBe(42);
    expect(driftCount()).toBe(1);
  });
});

// ---------------------------------------------------------------------------
// 通用读命令（`readFile`）的类型面：形参三元组（path / offset / limit）与
// `FileSlice | null` 结果类型逐字持衡；退役命令（`readExecOutput` / `readExplore`
// / `archiReadDoc`）已不在 `commands` 面（键集由上方 `commandKeys` 全量断言覆盖）。
// ---------------------------------------------------------------------------

describe('通用读命令：readFile 类型面与空态可分辨', () => {
  it('正向：形参元组与返回类型逐字相等（`[string, number | null, number | null]` / `FileSlice | null`）', () => {
    expectTypeOf<
      MutualAssignable<Parameters<typeof commands.readFile>, [string, number | null, number | null]>
    >().toEqualTypeOf<true>();
    expectTypeOf<
      MutualAssignable<Awaited<ReturnType<typeof commands.readFile>>, FileSlice | null>
    >().toEqualTypeOf<true>();
    expectTypeOf<
      MutualAssignable<z.infer<typeof resultSchemas.readFile>, FileSlice | null>
    >().toEqualTypeOf<true>();
    expectTypeOf<
      MutualAssignable<
        z.infer<typeof argsSchemas.readFile>,
        { path: string; offset: number | null; limit: number | null }
      >
    >().toEqualTypeOf<true>();
    expect(Object.keys((argsSchemas.readFile as ZodObject).shape)).toEqual([
      'path',
      'offset',
      'limit',
    ]);
  });

  it('异常：反例校准 —— 丢掉 `| null` 或改宽形参元组即判不等价（断言非恒真）', () => {
    expectTypeOf<
      MutualAssignable<Awaited<ReturnType<typeof commands.readFile>>, FileSlice>
    >().toEqualTypeOf<false>();
    expectTypeOf<
      MutualAssignable<Parameters<typeof commands.readFile>, [string]>
    >().toEqualTypeOf<false>();
    expectTypeOf<
      MutualAssignable<Parameters<typeof commands.readFile>, [string, number, number]>
    >().toEqualTypeOf<false>();
  });

  it('边界：空态（null）与脏结果在运行期可分辨（null 合法零漂移，异型 soft-report 原值透传）', async () => {
    invokeMock.mockResolvedValue(null);
    expect(await ipc.readFile('/ws/a.md', null, null)).toBeNull();
    expect(driftCount()).toBe(0);

    // 非 FileSlice / null 的脏结果只 soft-report，原值透传不 throw
    invokeMock.mockResolvedValue(42);
    expect(await ipc.readFile('/ws/a.md', null, null)).toBe(42);
    expect(driftCount()).toBe(1);

    // schema 面：null 合法、异型被拒
    const schema = resultSchemas.readFile as ZodType;
    expect(schema.safeParse(null).success).toBe(true);
    expect(schema.safeParse(42).success).toBe(false);
    // 游标与 stat 数值字段的 number 口径
    expectTypeOf<NonNullable<FileSlice>['nextOffset']>().toEqualTypeOf<number>();
    expectTypeOf<NonNullable<FileSlice>['stat']['size']>().toEqualTypeOf<number>();
  });
});
