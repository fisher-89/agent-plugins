import { ZodObject, ZodType, prettifyError } from 'zod';

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

/** jsonPointer 风格路径：`#` 表示根，`#/a/0/b` 逐段 `/`（`~`/`/` 按 RFC 6901 转义） */
function jsonPointer(segments: ReadonlyArray<string>): string {
  if (segments.length === 0) return '#';
  const escaped = segments.map(
    (segment) => `/${segment.replaceAll('~', '~0').replaceAll('/', '~1')}`,
  );
  return `#${escaped.join('')}`;
}

/** 从校验输入里取路径处的原值（`got`）：途中任一段不存在即 `undefined` */
function valueAtPath(value: unknown, segments: ReadonlyArray<string>): unknown {
  let current: unknown = value;
  for (const segment of segments) {
    if (typeof current !== 'object' || current === null) return undefined;
    current = Reflect.get(current, segment);
  }
  return current;
}

/** zod issue 的路径段（`path` 恒存在；段值统一收敛为字符串） */
function issueSegments(issue: unknown): ReadonlyArray<string> {
  if (typeof issue !== 'object' || issue === null || !('path' in issue)) return [];
  const { path } = issue;
  if (!Array.isArray(path)) return [];
  return path.map((segment: unknown) => String(segment));
}

/** zod issue 的 `expected`（仅类型错带该字段；其余回落 issue 文案） */
function issueExpected(issue: unknown): string {
  if (typeof issue !== 'object' || issue === null) return 'unknown';
  if ('expected' in issue && typeof issue.expected === 'string') return issue.expected;
  if ('message' in issue && typeof issue.message === 'string') return issue.message;
  return 'unknown';
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

/** 结果面 / 信封面 soft-report：失败只记录（含首个 issue 的路径与期望），永不 throw */
function reportMismatch(cmd: string, schema: ZodType, value: unknown): void {
  try {
    const result = schema.safeParse(value);
    if (result.success) return;
    const [first] = result.error.issues;
    const segments = issueSegments(first);
    report({
      cmd,
      path: jsonPointer(segments),
      expected: issueExpected(first),
      got: valueAtPath(value, segments),
    });
  } catch {
    // schema 自身异常（非契约不匹配）不得把业务路径变成 throw——降级为「不报告」
  }
}

/** 参数面表项：无该命令的 object schema 即跳过校验（fail-open，绝不误拒合法调用） */
function argsSchemaOf(cmd: string): ZodObject | null {
  const schema: unknown = Reflect.get(argsSchemas, cmd);
  return schema instanceof ZodObject ? schema : null;
}

/** 结果面表项 */
function resultSchemaOf(cmd: string): ZodType | null {
  const schema: unknown = Reflect.get(resultSchemas, cmd);
  return schema instanceof ZodType ? schema : null;
}

/** 信封面表项：`Channel` 形参名 → 信封 schema（无该命令的表项即无 Channel） */
function channelSchemasOf(cmd: string): Map<string, ZodType> | null {
  const table: unknown = Reflect.get(channelArgs, cmd);
  if (table === null || typeof table !== 'object') return null;
  const schemas = new Map<string, ZodType>();
  for (const [key, value] of Object.entries(table)) {
    if (value instanceof ZodType) schemas.set(key, value);
  }
  return schemas;
}

/** 校验输入：以 schema 的 shape 键顺（= Rust 形参序）把位置实参装配成 payload 副本 */
function argsPayloadOf(schema: ZodObject, args: ReadonlyArray<unknown>): Record<string, unknown> {
  const keys = Object.keys(schema.shape);
  return Object.fromEntries(keys.map((key, index) => [key, args[index]]));
}

/** 参数面 strict：失败抛 `IpcArgsError`（消息 = 命令名 + 全量 issues 的 prettify 文案） */
function validateArgs(cmd: string, args: ReadonlyArray<unknown>): void {
  const schema = argsSchemaOf(cmd);
  if (schema === null) return;
  const payload = argsPayloadOf(schema, args);
  try {
    const result = schema.safeParse(payload);
    if (result.success) return;
    const [first] = result.error.issues;
    const segments = issueSegments(first);
    throw new IpcArgsError(
      cmd,
      jsonPointer(segments),
      issueExpected(first),
      valueAtPath(payload, segments),
      prettifyError(result.error),
    );
  } catch (err: unknown) {
    // schema 自身异常也收敛为 `IpcArgsError`：本地参数面绝不把内部异常当业务 reject
    if (err instanceof IpcArgsError) throw err;
    throw new IpcArgsError(cmd, '#', 'valid input', payload, String(err));
  }
}

/** 已包裹的 Channel 实例（同一实例重复传入只包一次、只上报一次） */
const guardedChannels = new WeakSet<object>();

/** 原型上的 `onmessage` setter（`Channel` 的投递只读私有字段，故必须经 accessor 写入） */
function prototypeSetter(channel: object): Function | null {
  const descriptor: unknown = Object.getOwnPropertyDescriptor(
    Object.getPrototypeOf(channel),
    'onmessage',
  );
  if (typeof descriptor !== 'object' || descriptor === null) return null;
  const setter: unknown = Reflect.get(descriptor, 'set');
  return typeof setter === 'function' ? setter : null;
}

/**
 * 在原实例上装 `onmessage` 拦截：`set` 记住调用方 handler 并经**原型上的原 setter**
 * 把稳定 forwarder 装进 `Channel` 的私有字段；`get` 返回调用方 handler（原型无
 * accessor 时返回 forwarder，兼容测试替身）。其它可观察行为（`id` / `toJSON` /
 * 取消订阅 / 序号）零介入——只在原实例上加一个 accessor，不换实例、不改字段。
 */
function guardChannel(cmd: string, channel: object, schema: ZodType): void {
  if (guardedChannels.has(channel)) return;
  guardedChannels.add(channel);

  const setter = prototypeSetter(channel);
  let handler: unknown;
  let forwarder: ((message: unknown) => void) | null = null;
  const ensureForwarder = (): ((message: unknown) => void) => {
    forwarder ??= (message: unknown) => {
      reportMismatch(cmd, schema, message);
      if (typeof handler === 'function') Reflect.apply(handler, channel, [message]);
    };
    return forwarder;
  };

  Object.defineProperty(channel, 'onmessage', {
    configurable: true,
    get: () => (handler === undefined ? ensureForwarder() : handler),
    set: (next: unknown) => {
      handler = next;
      if (setter !== null) Reflect.apply(setter, channel, [ensureForwarder()]);
    },
  });
}

/** 信封面装配：生成物给出「哪些形参是 Channel」+ 信封 schema，不靠运行期值嗅探 */
function guardChannels(cmd: string, args: ReadonlyArray<unknown>): void {
  const schemas = channelSchemasOf(cmd);
  const schema = argsSchemaOf(cmd);
  if (schemas === null || schema === null) return;
  const keys = Object.keys(schema.shape);
  for (const [key, envelope] of schemas) {
    const value = args[keys.indexOf(key)];
    if (typeof value !== 'object' || value === null) continue;
    guardChannel(cmd, value, envelope);
  }
}

/** 结果面 soft-report：原值透传（零拷贝、零改写、零 throw） */
function softReportResult(cmd: string, result: unknown): unknown {
  const schema = resultSchemaOf(cmd);
  if (schema !== null) reportMismatch(cmd, schema, result);
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
