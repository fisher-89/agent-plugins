/**
 * IPC 类型过渡 shim：纯 re-export 生成绑定（`src/types/generated/bindings.ts`
 * 为唯一 IPC 类型事实源）。一次性切换的容错网——漏网旧 import 经此编译不炸，
 * knip unused 报告兜漏网名单；零引用即删（判据驱动，不按时间过渡）。
 * 本文件 MUST NOT 承载任何手写类型定义。
 */

export type * from './generated/bindings';
