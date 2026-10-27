import { type ZodError, type ZodIssue, ZodObject, ZodType, prettifyError, z } from 'zod';

import { commands } from '../types/generated/bindings';
import { argsSchemas, channelArgs, resultSchemas } from '../types/generated/bindings.zod';
import { type IpcDriftReport, driftCount, driftReports, reportDrift } from './ipc-drift';

/**
 * IPC 运行期校验装配层：命令集合与参数 / 结果形态**全部**由生成物推导
 * （`bindings.ts` 的 `commands` 键 + `bindings.zod.ts` 的三张表），零手写命令清单、
 * 零参数 / 结果类型镜像——新增命令零改本文件即自动具备两级校验。
 *
 * 分级策略（按方向）：
 * - **参数面**（前端 → Rust）strict：`safeParse` 失败 → 本地抛 {@link IpcArgsError}，
 *   **零 IPC 往返**；通过后以**原参数对象**发起调用（`Channel` 等非 JSON 可表达参数
 *   原样留在 payload 内，「跳过校验 ≠ 剔出对象」）。
 * - **结果面**（Rust → 前端）soft-report：失败只记一条漂移并**原值透传**，既有手写
 *   降级渲染照旧，UI 不崩。
 * - **Channel 信封**：逐条 soft-report，原消息原样交付 handler（不丢流、不断流），
 *   同一 `Channel` 实例重复传入幂等（`WeakSet` 短路）。
 *
 * 错误面三分：`invoke` reject（后端业务失败，原样向上抛）/ `IpcArgsError`（本地参数
 * 拒绝，`extends Error` 故既有 `String(err)` 呈现零改动）/ 漂移报告（只上报，永不 throw）。
 *
 * zod 面一律用**类型化的 issue**（`ZodIssue`）：`path` / `code` / `expected` 都是声明面
 * 的一部分，不再对 issue 做 `unknown` 收窄——那种「防御式再判空」在真实 zod 产物下
 * 恒为真，只会让语义被两层等价守卫遮住。
 */

/** 本地参数拒绝：`extends Error` 使既有「`String(err)` 进 error 态」链路零改动 */
class IpcArgsError extends Error {
  readonly cmd: string;
  readonly path: string;
  readonly expected: string;
  readonly got: unknown;

  constructor(cmd: string, path: string, expected: string, got: unknown, detail: string) {
    super(`命令 ${cmd} 参数校验失败：${detail}`);
    this.name = 'IpcArgsError';
    this.cmd = cmd;
    this.path = path;
    this.expected = expected;
    this.got = got;
  }
}

/**
 * 表项缺失时的放行面（fail-open）：生成物逐命令都有表项，此处只为「绝不误拒合法调用」
 * ——参数面退化为「空形参」（接受任意调用）、结果面退化为「不做观测」。
 */
const NO_ARGS_SCHEMA = z.object({});
const NO_RESULT_SCHEMA: ZodType = z.unknown();

/** jsonPointer 风格路径：`#` 表示根，`#/a/0/b` 逐段 `/` */
function jsonPointer(segments: ReadonlyArray<string>): string {
  return `#${segments.map((segment) => `/${segment}`).join('')}`;
}

/**
 * 路径处原值（`got`）：`Object(node)` 把中途遇到的标量装箱，于是缺失键 / 标量段自然
 * 得 `undefined` 而不是 `Reflect` 的类型错误——提取 `got` 的手必须永不抛。
 */
function valueAtPath(value: unknown, segments: ReadonlyArray<string>): unknown {
  return segments.reduce<unknown>((node, segment) => Reflect.get(Object(node), segment), value);
}

/** zod issue 的 `expected`：仅类型错带该字段，其余取 issue 文案 */
function issueExpected(issue: ZodIssue): string {
  return issue.code === 'invalid_type' ? issue.expected : issue.message;
}

/** 单条漂移上报：先取计数 → 记录 → 计数变化（新去重键）时在 DEV 打一次汇总告警 */
function report(report: IpcDriftReport): void {
  const before = driftCount();
  reportDrift(report);
  const after = driftCount();
  if (import.meta.env.DEV !== true || after === before) return;
  console.warn(
    `[ipc-drift] ${report.cmd} ${report.path} 漂移（expected ${report.expected}）；` +
      `当前去重键 ${after} 个`,
    driftReports(),
  );
}

/** `safeParse` 的失败面：通过或 schema 自身抛异常均 `null`（观测链路绝不把内部异常当漂移） */
function parseFailure(schema: ZodType, value: unknown): ZodError | null {
  try {
    const { success, error } = schema.safeParse(value);
    return success ? null : error;
  } catch {
    return null;
  }
}

/** 结果面 / 信封面 soft-report：失败只记录（含首个 issue 的路径与期望），永不 throw */
function reportMismatch(cmd: string, schema: ZodType, value: unknown): void {
  const failure = parseFailure(schema, value);
  if (failure === null) return;
  const [issue] = failure.issues;
  const segments = issue.path.map(String);
  report({
    cmd,
    path: jsonPointer(segments),
    expected: issueExpected(issue),
    got: valueAtPath(value, segments),
  });
}

/** 参数面表项（形状 = 形参名与形参序）：`instanceof ZodObject` 是类型收窄 + 放行面 */
function argsSchemaOf(cmd: string): ZodObject {
  const schema: unknown = Reflect.get(argsSchemas, cmd);
  return schema instanceof ZodObject ? schema : NO_ARGS_SCHEMA;
}

/** 结果面表项（`Result` 的 Ok 臂）：`instanceof ZodType` 是类型收窄 + 放行面 */
function resultSchemaOf(cmd: string): ZodType {
  const schema: unknown = Reflect.get(resultSchemas, cmd);
  return schema instanceof ZodType ? schema : NO_RESULT_SCHEMA;
}

/** 信封面表项：`Channel` 形参名 → 信封 schema（生成物逐命令都有表项，无 Channel 时为 `{}`） */
function channelSchemasOf(cmd: string): ReadonlyMap<string, ZodType> {
  return new Map(Object.entries<ZodType>(Reflect.get(channelArgs, cmd)));
}

/** 校验输入：以 schema 的 shape 键顺（= Rust 形参序）把位置实参装配成 payload 副本 */
function argsPayloadOf(schema: ZodObject, args: ReadonlyArray<unknown>): Record<string, unknown> {
  const keys = Object.keys(schema.shape);
  return Object.fromEntries(keys.map((key, index) => [key, args[index]]));
}

/** 首个 issue 的本地拒绝：命令名 + jsonPointer 路径 + expected/got + 全量 issues 文案 */
function argsErrorOf(cmd: string, payload: Record<string, unknown>, error: ZodError): IpcArgsError {
  const [first] = error.issues;
  const segments = first.path.map(String);
  return new IpcArgsError(
    cmd,
    jsonPointer(segments),
    issueExpected(first),
    valueAtPath(payload, segments),
    prettifyError(error),
  );
}

/** 参数面 strict：失败抛 `IpcArgsError`（零 IPC 往返），schema 自身异常同样收敛为本地拒绝 */
function validateArgs(cmd: string, args: ReadonlyArray<unknown>): void {
  const schema = argsSchemaOf(cmd);
  const payload = argsPayloadOf(schema, args);
  let failure: ZodError | null = null;
  try {
    const outcome = schema.safeParse(payload);
    failure = outcome.success ? null : outcome.error;
  } catch (err: unknown) {
    throw new IpcArgsError(cmd, '#', 'valid input', payload, String(err));
  }
  if (failure !== null) throw argsErrorOf(cmd, payload, failure);
}

/** 已包裹的 Channel 实例（同一实例重复传入只包一次、只上报一次） */
const guardedChannels = new WeakSet<object>();

/** 原型上的 `onmessage` descriptor（`Channel` 的投递只读私有字段，故必须经原型 accessor 写入） */
function prototypeDescriptor(channel: object): PropertyDescriptor | undefined {
  return Object.getOwnPropertyDescriptor(Object.getPrototypeOf(channel), 'onmessage');
}

/**
 * 在原实例上装 `onmessage` 拦截：`get` 返回调用方 handler（未接线时返回 forwarder），
 * `set` 只记住 handler；稳定 forwarder 经**原型上的原 setter** 装进 `Channel` 的私有
 * 字段，投递路径即 `validate → reportDrift → handler(msg)`。其它可观察行为（`id` /
 * `toJSON` / 取消订阅 / 序号）零介入——只在原实例上加一个 accessor，不换实例、不改字段。
 *
 * 安装时**接住已接线的 handler**（真实消费形态是先 `onmessage = handler` 再传
 * `channel`，见 `agent-transport` / `use-change-flow-run` / `use-explore-doc`）：
 * 只有先把当前 `onmessage` 读进 `handler` 并立刻用 forwarder 覆盖私有字段，逐条校验
 * 才真正落在交付路径上（否则私有字段仍是调用方 handler，信封绕过校验）。handler 之后
 * 再被改写只会改变 forwarder 读取的那个变量，无需重装私有字段。
 */
function guardChannel(cmd: string, channel: object, schema: ZodType): void {
  if (guardedChannels.has(channel)) return;
  guardedChannels.add(channel);

  const inherited = prototypeDescriptor(channel);
  // 原型 setter 只能以函数值取用（从 descriptor 直接取成员会被判为 unbound method）
  const setter: unknown = Reflect.get(inherited ?? {}, 'set');
  // 必须在自身 `defineProperty` 之前读：否则读到的是本函数的 getter
  let handler: unknown = Reflect.get(channel, 'onmessage');
  const forwarder = (message: unknown): void => {
    reportMismatch(cmd, schema, message);
    if (typeof handler === 'function') Reflect.apply(handler, channel, [message]);
  };

  Object.defineProperty(channel, 'onmessage', {
    ...inherited,
    get: () => (typeof handler === 'function' ? handler : forwarder),
    set: (next: unknown) => {
      handler = next;
    },
  });
  // 原型带 accessor（真实 `Channel` / 同构替身）：投递直读私有字段，故把 forwarder 写进去
  if (typeof setter === 'function') Reflect.apply(setter, channel, [forwarder]);
}

/** 信封面装配：生成物给出「哪些形参是 Channel」+ 信封 schema，不靠运行期值嗅探 */
function guardChannels(cmd: string, args: ReadonlyArray<unknown>): void {
  const keys = Object.keys(argsSchemaOf(cmd).shape);
  for (const [key, envelope] of channelSchemasOf(cmd)) {
    const value = args[keys.indexOf(key)];
    if (typeof value !== 'object' || value === null) continue;
    guardChannel(cmd, value, envelope);
  }
}

/** 结果面 soft-report：原值透传（零拷贝、零改写、零 throw） */
function softReportResult(cmd: string, result: unknown): unknown {
  reportMismatch(cmd, resultSchemaOf(cmd), result);
  return result;
}

/** 单命令调用：参数面 strict → Channel 包裹 → 原参数对象 invoke → 结果面 soft-report */
async function invokeGuarded(
  cmd: string,
  command: Function,
  args: ReadonlyArray<unknown>,
): Promise<unknown> {
  validateArgs(cmd, args);
  guardChannels(cmd, args);
  const result: unknown = await Reflect.apply(command, undefined, args);
  return softReportResult(cmd, result);
}

/** 按命令名缓存的包裹函数（`ipc.x === ipc.x`，不制造 React 依赖抖动） */
const guards = new Map<string, unknown>();

/** 分级校验后的调用面：键集合与签名完全来自 `commands`，新增命令零改本文件 */
export const ipc: typeof commands = new Proxy(commands, {
  get(target, property) {
    if (typeof property !== 'string' || !Object.hasOwn(target, property)) {
      return Reflect.get(target, property);
    }
    const command: unknown = Reflect.get(target, property);
    if (typeof command !== 'function') return command;
    const cached = guards.get(property);
    if (cached !== undefined) return cached;
    const guarded = (...args: unknown[]) => invokeGuarded(property, command, args);
    guards.set(property, guarded);
    return guarded;
  },
});
