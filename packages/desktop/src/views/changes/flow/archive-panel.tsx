/**
 * 归档面板（design D13/D14 + archive-merge-first D11）：确认对话（完成度警告
 * 清单不阻断 + delta specs 清单 + worktree 合入告知与目标分支名 + 跳过同步
 * checkbox + runActive 拒绝卡）→ 进行面（六段阶段清单——呈现序 = 执行序：
 * 校验 → 提交 → 合入 → 同步 → 双写 → 落盘；合入段子阶段 = 单行内状态与
 * detail 演进——冲突解算中 / 已解冲突 / lean 咨询串均经既有 detail 透传，
 * 零子阶段行；停止 + agent 转录入口——复用 `useSessionTranscript` +
 * `AgentTimeline` 直组，转录随当前会话切换零切换器）→ 终态面（结果摘要 /
 * 错误 + 幂等重试）。面板纯呈现面：状态与动作经 `use-archive-flow` 注入；
 * 失败重试直发 start（全阶段幂等——重试快速空走到未完成阶段续走）。
 */
import { useEffect, useState } from 'react';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';

import { AgentTimeline } from '../../../components/agent';
import type {
  AgentEvent,
  ArchivePreflight,
  ArchiveSpecsStatus,
  ArchiveStage,
  ArchiveStageState,
  ArchiveStageStatus,
  ArchiveSummary,
} from '../../../types/dto';
import type { UseArchiveFlowResult } from '../hooks/use-archive-flow';
import { useSessionTranscript } from '../hooks/use-session-transcript';
import { ARCHIVE_STAGES } from './archive-state';

/** 阶段行文案（六段）。 */
const STAGE_LABEL: Record<ArchiveStage, string> = {
  preflight: '前置校验',
  specSync: 'spec 同步',
  commit: 'worktree 提交',
  merge: '主仓合入',
  seal: '归档收口',
  finalize: '归档落盘',
};

/** 阶段状态徽章文案与样式。 */
const STAGE_STATUS_LABEL: Record<ArchiveStageStatus, string> = {
  running: '运行中',
  passed: '通过',
  skipped: '跳过',
  failed: '失败',
};

const STAGE_STATUS_VARIANT: Record<ArchiveStageStatus, 'secondary' | 'pass' | 'ghost' | 'fail'> = {
  running: 'secondary',
  passed: 'pass',
  skipped: 'ghost',
  failed: 'fail',
};

/** 摘要 specs 行三态文案（skill Output On Success specs 行对译）。 */
const SPECS_LABEL: Record<ArchiveSpecsStatus, string> = {
  synced: '已同步 delta specs',
  skipped: '跳过 spec 同步（用户选择）',
  none: '无 delta specs',
};

interface ArchivePanelProps {
  root: string | null;
  change: string;
  archive: UseArchiveFlowResult;
  /** 归档面板开合（DetailHeader 归档按钮触发；取消 / 关闭回落） */
  open: boolean;
  onClose: () => void;
}

/** 归档面板三分面：确认对话（无链状态）→ 进行面 → 终态面。 */
export function ArchivePanel({
  root,
  change,
  archive,
  open,
  onClose,
}: ArchivePanelProps): React.JSX.Element | null {
  // 最近一次确认的同步选择（终态失败重试沿用——链幂等续走无需再次确认）
  const [confirmedSyncSpecs, setConfirmedSyncSpecs] = useState(true);
  const startWith = (syncSpecs: boolean): void => {
    setConfirmedSyncSpecs(syncSpecs);
    void archive.start(syncSpecs);
  };
  if (!open) return null;
  const state = archive.state;
  if (state === null) {
    return (
      <ConfirmDialogFace
        change={change}
        archive={archive}
        onConfirm={startWith}
        onClose={onClose}
      />
    );
  }
  if (!state.finished) {
    return (
      <ProgressFace
        root={root}
        change={change}
        archive={archive}
        sessionId={state.sessionId}
        liveEvents={state.liveEvents[state.sessionId ?? ''] ?? []}
      />
    );
  }
  return (
    <TerminalFace
      archive={archive}
      onRetry={() => void archive.start(confirmedSyncSpecs)}
      onClose={onClose}
    />
  );
}

// ---------------------------------------------------------------------------
// 确认对话面（design D13：警告不阻断——inform + confirm；runActive 拒绝卡）
// ---------------------------------------------------------------------------

/** 确认对话数据警告行组装（完成度 + 产物缺失）。 */
function dialogWarnings(data: ArchivePreflight): string[] {
  const warnings: string[] = [];
  if (!data.completed) {
    warnings.push(
      data.incompletePhases.length > 0
        ? `工作流未全部通过：${data.incompletePhases.join('、')}`
        : '完成度不可核算（工作流类型无相位表）',
    );
  }
  if (data.missingArtifacts.length > 0) {
    warnings.push(`缺少产物文档：${data.missingArtifacts.join('、')}`);
  }
  return warnings;
}

/** 警告行组（informs，不阻断确认）。 */
function WarningRows({ warnings }: { warnings: string[] }): React.JSX.Element | null {
  if (warnings.length === 0) return null;
  return (
    <div className="flex flex-col gap-1">
      {warnings.map((warning) => (
        <div
          key={warning}
          className="break-all rounded-md bg-warn-bg px-2.5 py-1.5 text-[13px] text-warn"
          data-testid="archive-warning"
        >
          {warning}
        </div>
      ))}
    </div>
  );
}

/** delta specs capability 清单 + 跳过同步 checkbox（默认不勾；缺席整块隐藏）。 */
function DeltaSpecsChoice({
  capabilities,
  skipSync,
  onSkipSyncChange,
}: {
  capabilities: string[];
  skipSync: boolean;
  onSkipSyncChange: (checked: boolean) => void;
}): React.JSX.Element {
  return (
    <div className="text-[13px]" data-testid="archive-delta-specs">
      delta specs（{capabilities.length} 个 capability）：{capabilities.join('、')}
      <label className="mt-1 flex items-center gap-1.5">
        <input
          type="checkbox"
          checked={skipSync}
          onChange={(event) => onSkipSyncChange(event.target.checked)}
          data-testid="archive-sync-skip"
        />
        跳过 delta specs 同步，直接归档
      </label>
    </div>
  );
}

/** worktree 合入告知（目标 = 主仓当前分支；探测失败占位——R4 呈现消解）。 */
function MergeNotice({ mergeTarget }: { mergeTarget: string | null }): React.JSX.Element {
  return (
    <div className="break-all text-[13px]" data-testid="archive-merge-notice">
      将提交 worktree 并合入主仓当前分支：
      <span className="text-muted-foreground">{mergeTarget ?? '（当前分支探测失败）'}</span>
    </div>
  );
}

/** 确认按钮行 / runActive 拒绝卡二选一（拒绝卡呈现运行中原因——AC-1）。 */
function ConfirmActions({
  runActive,
  onConfirm,
  onClose,
}: {
  runActive: boolean;
  onConfirm: () => void;
  onClose: () => void;
}): React.JSX.Element {
  if (runActive) {
    return (
      <div
        className="break-all rounded-md bg-warn-bg px-2.5 py-1.5 text-[13px] text-warn"
        data-testid="archive-run-active"
      >
        该 change 存在运行中的 run，不可发起归档——请先停止 run（或等待收口）。
      </div>
    );
  }
  return (
    <div className="flex gap-2">
      <Button onClick={onConfirm} data-testid="archive-confirm-ok">
        确认归档
      </Button>
      <Button onClick={onClose} data-testid="archive-confirm-cancel">
        取消
      </Button>
    </div>
  );
}

/** 确认对话数据面组装（警告 + delta specs + 合入告知 + 动作行）。 */
function ConfirmBody({
  data,
  skipSync,
  onSkipSyncChange,
  onConfirm,
  onClose,
}: {
  data: ArchivePreflight;
  skipSync: boolean;
  onSkipSyncChange: (checked: boolean) => void;
  onConfirm: (syncSpecs: boolean) => void;
  onClose: () => void;
}): React.JSX.Element {
  return (
    <div className="flex flex-col gap-2">
      <WarningRows warnings={dialogWarnings(data)} />
      {data.deltaSpecs.length > 0 && (
        <DeltaSpecsChoice
          capabilities={data.deltaSpecs}
          skipSync={skipSync}
          onSkipSyncChange={onSkipSyncChange}
        />
      )}
      {data.worktree !== null && <MergeNotice mergeTarget={data.mergeTarget} />}
      <ConfirmActions
        runActive={data.runActive}
        onConfirm={() => onConfirm(!skipSync)}
        onClose={onClose}
      />
    </div>
  );
}

/** 确认对话外壳（标题 + 数据面插槽）。 */
function ConfirmShell({
  change,
  children,
}: {
  change: string;
  children: React.ReactNode;
}): React.JSX.Element {
  return (
    <section
      className="mb-4 rounded-lg border border-warn bg-card px-4 py-3.5"
      data-testid="archive-confirm-dialog"
      aria-label={`change ${change} 归档确认`}
    >
      <h2 className="m-0 mb-2.5 text-[15px]">归档 change「{change}」</h2>
      {children}
    </section>
  );
}

function ConfirmDialogFace({
  change,
  archive,
  onConfirm,
  onClose,
}: {
  change: string;
  archive: UseArchiveFlowResult;
  onConfirm: (syncSpecs: boolean) => void;
  onClose: () => void;
}): React.JSX.Element {
  // undefined = 取数中；null = 不可归档（读面兜底）
  const [data, setData] = useState<ArchivePreflight | null | undefined>(undefined);
  const [skipSync, setSkipSync] = useState(false);
  const { preflight } = archive;

  useEffect(() => {
    let disposed = false;
    void preflight().then((result) => {
      if (!disposed) setData(result ?? null);
    });
    return () => {
      disposed = true;
    };
  }, [preflight]);

  let body: React.JSX.Element;
  if (data === undefined) {
    body = <div className="text-[13px] text-muted-foreground">正在核对归档前置…</div>;
  } else if (data === null) {
    body = (
      <div className="text-[13px] text-muted-foreground" data-testid="archive-unavailable">
        该 change 当前不可归档（未建档 / 已归档 / 未知名）。
      </div>
    );
  } else {
    body = (
      <ConfirmBody
        data={data}
        skipSync={skipSync}
        onSkipSyncChange={setSkipSync}
        onConfirm={onConfirm}
        onClose={onClose}
      />
    );
  }
  return <ConfirmShell change={change}>{body}</ConfirmShell>;
}

// ---------------------------------------------------------------------------
// 进行面
// ---------------------------------------------------------------------------

/** 六段阶段清单（固定行序 = 执行序——archive-merge-first D1；合入段子阶段
 * 经同段后写覆盖的 detail 演进呈现，零子阶段行；`data-status` 供测试读态）。 */
function StageList({
  stages,
}: {
  stages: Partial<Record<ArchiveStage, ArchiveStageState>>;
}): React.JSX.Element {
  return (
    <div className="mt-2 flex flex-col gap-1" data-testid="archive-stage-list">
      {ARCHIVE_STAGES.map((stage) => {
        const row = stages[stage];
        return (
          <div
            key={stage}
            className="flex flex-wrap items-center gap-2 text-[13px]"
            data-testid={`archive-stage-${stage}`}
            data-status={row?.status ?? 'pending'}
          >
            <span>{STAGE_LABEL[stage]}</span>
            {row === undefined ? (
              <span className="text-muted-foreground">待执行</span>
            ) : (
              <>
                <Badge variant={STAGE_STATUS_VARIANT[row.status]}>
                  {STAGE_STATUS_LABEL[row.status]}
                </Badge>
                {row.detail !== null && (
                  <span className="break-all text-muted-foreground">{row.detail}</span>
                )}
              </>
            )}
          </div>
        );
      })}
    </div>
  );
}

function ProgressFace({
  root,
  change,
  archive,
  sessionId,
  liveEvents,
}: {
  root: string | null;
  change: string;
  archive: UseArchiveFlowResult;
  sessionId: string | null;
  liveEvents: AgentEvent[];
}): React.JSX.Element {
  const { state, stop, error } = archive;
  return (
    <section
      className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5"
      data-testid="archive-progress"
      aria-label={`change ${change} 归档进行中`}
    >
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h2 className="m-0 text-[15px]">归档进行中</h2>
        <Button onClick={() => void stop()} data-testid="archive-stop">
          停止
        </Button>
      </div>
      {error !== null && (
        <div
          className="mt-2 break-all rounded-md bg-fail-bg px-2.5 py-1.5 text-[13px] text-fail"
          data-testid="archive-invoke-error"
        >
          {error}
        </div>
      )}
      <StageList stages={state?.stages ?? {}} />
      {sessionId !== null && (
        <ArchiveTranscript root={root} sessionId={sessionId} liveEvents={liveEvents} />
      )}
    </section>
  );
}

/** 归档 agent 会话转录入口（`useSessionTranscript` + `AgentTimeline` 直组——
 * 库内重放 + live 事件按 seq 归并，与既有装配同源语义；归档链两会话（解冲突
 * → spec 同步）严格串行，本面只呈现当前会话——D5 单槽复用，已收口会话经
 * 密封转录按 source_ref 反查回放，前端不设会话切换器）。 */
function ArchiveTranscript({
  root,
  sessionId,
  liveEvents,
}: {
  root: string | null;
  sessionId: string;
  liveEvents: AgentEvent[];
}): React.JSX.Element {
  const { messages, running, error } = useSessionTranscript({
    root,
    sourceRef: null,
    sessionId,
    liveEvents,
  });
  return (
    <div className="mt-2.5" data-testid="archive-transcript">
      <h3 className="m-0 mb-1.5 text-[13px] text-muted-foreground">同步会话转录</h3>
      {error !== null && (
        <div className="mb-2 break-all text-[13px] text-fail" data-testid="transcript-error">
          {error}
        </div>
      )}
      {messages.length === 0 && error === null ? (
        <div className="text-[13px] text-muted-foreground" data-testid="transcript-empty">
          （暂无同步会话转录）
        </div>
      ) : (
        <div className="max-h-64 overflow-y-auto rounded-md border border-border bg-background px-3 py-2">
          <AgentTimeline messages={messages} running={running} />
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// 终态面
// ---------------------------------------------------------------------------

/** 结果摘要（名 / 归档位置 / specs 行三态 / 警告清单——skill Output 对译）。 */
function SummaryFace({
  summary,
  onClose,
}: {
  summary: ArchiveSummary;
  onClose: () => void;
}): React.JSX.Element {
  return (
    <section
      className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5"
      data-testid="archive-summary"
    >
      <h2 className="m-0 mb-2.5 text-[15px]">归档完成</h2>
      <div className="flex flex-col gap-1 text-[13px]">
        <div data-testid="archive-summary-name">change：{summary.name}</div>
        <div data-testid="archive-summary-dir" className="break-all">
          归档位置：{summary.archivedDir}
        </div>
        <div data-testid="archive-specs-line">{SPECS_LABEL[summary.specs]}</div>
        {summary.warnings.map((warning) => (
          <div
            key={warning}
            className="break-all rounded-md bg-warn-bg px-2.5 py-1.5 text-warn"
            data-testid="archive-warning"
          >
            {warning}
          </div>
        ))}
      </div>
      <div className="mt-2.5">
        <Button onClick={onClose} data-testid="archive-close">
          关闭
        </Button>
      </div>
    </section>
  );
}

/** 失败 / 停止面（错误记因 + 幂等重试 / 关闭）。 */
function FailureFace({
  error,
  onRetry,
  onClose,
}: {
  error: string;
  onRetry: () => void;
  onClose: () => void;
}): React.JSX.Element {
  return (
    <section
      className="mb-4 rounded-lg border border-fail bg-card px-4 py-3.5"
      data-testid="archive-terminal-error"
    >
      <h2 className="m-0 mb-2.5 text-[15px]">归档未完成</h2>
      <div
        className="break-all rounded-md bg-fail-bg px-2.5 py-1.5 text-[13px] text-fail"
        data-testid="archive-error"
      >
        {error}
      </div>
      <div className="mt-2.5 flex gap-2">
        <Button onClick={onRetry} data-testid="archive-retry">
          重试归档
        </Button>
        <Button onClick={onClose} data-testid="archive-close">
          关闭
        </Button>
      </div>
    </section>
  );
}

function TerminalFace({
  archive,
  onRetry,
  onClose,
}: {
  archive: UseArchiveFlowResult;
  onRetry: () => void;
  onClose: () => void;
}): React.JSX.Element {
  const summary = archive.state?.summary ?? null;
  if (summary !== null) {
    return <SummaryFace summary={summary} onClose={onClose} />;
  }
  return (
    <FailureFace
      error={archive.state?.error ?? '归档链已停止'}
      onRetry={onRetry}
      onClose={onClose}
    />
  );
}
