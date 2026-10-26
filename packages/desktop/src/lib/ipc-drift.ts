/**
 * IPC 运行期漂移聚合（进程内、零持久化）：`(cmd, path)` 去重 + 计数 + 条数上限。
 *
 * 纯数据模块：零 `console`、零 IO、零 IPC——告警由装配层（`src/lib/ipc.ts`）在
 * 消费处打印，故本模块可被直接断言。生命周期与「当前 workspace 根」绑定：
 * 根**变更**时由 `src/hooks/use-workspaces.ts` 调 {@link resetDrift}（跨库混计会让
 * 「哪个库的数据漂移」失真；同根重取数不清）。
 */

/** 一条漂移报告载荷（不是 throw——结果面 / Channel 信封漂移只上报） */
export interface IpcDriftReport {
  /** 生成的命令 accessor 名（`commands` 的键，如 `agentStart`） */
  readonly cmd: string;
  /** jsonPointer 风格字段路径（`#` 根，`#/a/0/b` 逐段 `/`） */
  readonly path: string;
  /** 契约侧的期望形态（zod issue 的 `expected`，缺省回落 issue 文案） */
  readonly expected: string;
  /** 该路径上的实际值 */
  readonly got: unknown;
}

/** 聚合条目：去重键 + 累计次数 */
export interface DriftEntry extends IpcDriftReport {
  readonly count: number;
}

/**
 * 去重键上限：远大于真实漂移面（一屏 UI 的字段数级），又能封住无界增长。
 * 达上限后**丢弃新键**（已有键继续累计）——噪声封顶不该掩盖既有漂移。
 */
const MAX_DRIFT_KEYS = 200;

/** 键分隔符：命令名与 jsonPointer 路径都不含 NUL，故拼接无歧义 */
const KEY_SEPARATOR = '\u0000';

const entries = new Map<string, { report: IpcDriftReport; count: number }>();

function keyOf(report: IpcDriftReport): string {
  return `${report.cmd}${KEY_SEPARATOR}${report.path}`;
}

/**
 * 唯一写入口：同 `(cmd, path)` 累计计数，新键在未达上限时入表。
 * 消费方：`src/lib/ipc.ts`（参数面 / 结果面 / Channel 信封）。
 */
export function reportDrift(report: IpcDriftReport): void {
  const key = keyOf(report);
  const existing = entries.get(key);
  if (existing !== undefined) {
    entries.set(key, { report: existing.report, count: existing.count + 1 });
    return;
  }
  if (entries.size >= MAX_DRIFT_KEYS) return;
  entries.set(key, { report, count: 1 });
}

/**
 * 当前聚合快照（`readonly`，调用方改不动聚合本体）。
 * 消费方：`src/lib/ipc.ts` 的 DEV 汇总告警（列出当前键）。
 */
export function driftReports(): ReadonlyArray<DriftEntry> {
  return [...entries.values()].map(({ report, count }) => ({ ...report, count }));
}

/** 去重后的键数（非投递次数）。消费方：`src/lib/ipc.ts` 的 DEV 汇总告警。 */
export function driftCount(): number {
  return entries.size;
}

/**
 * 清空聚合。消费方：`src/hooks/use-workspaces.ts`（当前根**变更**时；
 * 同一根的重取数不清空）。
 */
export function resetDrift(): void {
  entries.clear();
}
