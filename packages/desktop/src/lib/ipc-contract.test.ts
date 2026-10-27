import type { Channel } from '@tauri-apps/api/core';
import { describe, expect, expectTypeOf, it } from 'vite-plus/test';
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
import type { ipc } from './ipc';

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
