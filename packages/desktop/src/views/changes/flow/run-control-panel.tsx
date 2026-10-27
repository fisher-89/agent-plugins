/**
 * 运行控制面板：发起 / 停止 / phase 间确认（继续 / 终止）/ ask 中断卡片
 *（问题 + 选项 + 自由文本应答）。状态面 = activeRun 活面（六值状态机 /
 * 停等卡片自统一查询 activeRun）∪ lastRun runs 尾行（收口后终态徽章与收口
 * 记因——unify-run-state-persistence D9：头部「上次运行」信息面经 runs 尾
 * 行可达，activeRun 退场后呈现）；生命周期状态与可用操作对齐——运行中主操
 * 作是停止，终局后主操作回到发起。
 */
import { useEffect, useState } from 'react';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';

import type {
  ActiveRunView,
  ArchivePreflight,
  AskPayload,
  ArchiChangeSummary,
  ChangeRunEntry,
  ChangeRunStatus,
  RunStatus,
} from '../../../types/dto';
import { commands } from '../../../types/generated/bindings';
import { ArchitectureCanvas } from '../../architecture/components/architecture-canvas';
import type { UseChangeFlowRunResult } from '../hooks/use-change-flow-run';

/** 活面状态徽章文案（六档状态机镜像）。 */
const ACTIVE_STATUS_LABEL: Record<ChangeRunStatus, string> = {
  running: '运行中',
  waitingConfirm: '等待确认',
  waitingAsk: '等待应答',
  completed: '已完成',
  stopped: '已停止',
  failed: '失败',
};

/** 库读史终态徽章文案（RunStatus 五值；interrupted 仅启动标定产生）。 */
const LAST_RUN_STATUS_LABEL: Record<RunStatus, string> = {
  running: '运行中',
  completed: '已完成',
  stopped: '已停止',
  failed: '失败',
  interrupted: '已中断',
};

/** 动作面（useChangeFlowRun 缩位返回型直用）。 */
type RunActions = Pick<UseChangeFlowRunResult, 'start' | 'stop' | 'confirm' | 'answer' | 'error'>;

interface RunControlPanelProps {
  /** change id（命令面透传——动作经 useChangeFlowRun 的 id 链路；架构变更区块独立取数键） */
  changeId: string;
  /** change 名（展示面：aria-label / 文案取 `detail.name`） */
  name: string;
  /** 主仓根（归档确认面的架构变更区块据它开库取数；null = 壳态无根） */
  root: string | null;
  /** 在飞 run 活面（统一视图 activeRun；终态即除名 → null） */
  activeRun: ActiveRunView | null;
  /** runs 尾行（库读史最近一次 run——收口后终态徽章与收口记因） */
  lastRun: ChangeRunEntry | null;
  actions: RunActions;
}

/** 发起 / 停止主操作行 */
function RunActions({
  actions,
  active,
}: {
  actions: RunActions;
  active: boolean;
}): React.JSX.Element {
  const [autoNextPhase, setAutoNextPhase] = useState(true);
  return (
    <div className="flex flex-col items-end gap-1">
      <div className="flex items-center gap-2">
        <label className="flex items-center gap-1.5 text-[13px]">
          <input
            type="checkbox"
            checked={autoNextPhase}
            disabled={active}
            onChange={(event) => setAutoNextPhase(event.target.checked)}
            data-testid="run-auto-next-phase"
          />
          连续运行
        </label>
        {active ? (
          <Button onClick={() => void actions.stop()} data-testid="run-stop">
            停止
          </Button>
        ) : (
          <Button onClick={() => void actions.start(autoNextPhase)} data-testid="run-start">
            发起运行
          </Button>
        )}
      </div>
    </div>
  );
}

/**
 * 停等确认卡片单入口：`phase === 'archive'` → 归档前停等卡片（stop-before-archive
 * 手动门，携归档前置载荷）；其余相位 → 既有个 phase 间确认卡片（proceed=false →
 * 受控终态 stopped）。
 */
function ConfirmCard({
  actions,
  phase,
  preflight,
  root,
  changeId,
}: {
  actions: RunActions;
  phase: string;
  preflight: ArchivePreflight | null;
  root: string | null;
  changeId: string | null;
}): React.JSX.Element {
  if (phase === 'archive') {
    return (
      <ArchiveConfirmCard actions={actions} preflight={preflight} root={root} changeId={changeId} />
    );
  }
  return (
    <div
      className="mt-2 rounded-md border border-border bg-card px-3 py-2"
      data-testid="run-confirm-card"
    >
      <div className="text-[13px]">
        相位 <span className="text-muted-foreground">{phase}</span> 已收口，是否继续推进下一相位？
      </div>
      <div className="mt-2 flex gap-2">
        <Button onClick={() => void actions.confirm(true, true)} data-testid="run-confirm-proceed">
          继续
        </Button>
        <Button onClick={() => void actions.confirm(false, true)} data-testid="run-confirm-stop">
          终止运行
        </Button>
      </div>
    </div>
  );
}

/** 归档前置警告行（完成度 + 产物缺失；inform 不阻断）。 */
function archiveWarnings(data: ArchivePreflight | null): string[] {
  if (data === null) return [];
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

/** 归档警告行组（informs，不阻断确认）。 */
function ArchiveWarningRows({ warnings }: { warnings: string[] }): React.JSX.Element | null {
  if (warnings.length === 0) return null;
  return (
    <div className="flex flex-col gap-1">
      {warnings.map((warning) => (
        <div
          key={warning}
          className="mt-1 break-all rounded-md bg-warn-bg px-2.5 py-1.5 text-[13px] text-warn"
          data-testid="run-archive-warning"
        >
          {warning}
        </div>
      ))}
    </div>
  );
}

/** delta specs capability 清单行（缺席整块隐藏）。 */
function ArchiveDeltaSpecs({ capabilities }: { capabilities: string[] }): React.JSX.Element | null {
  if (capabilities.length === 0) return null;
  return (
    <div className="mt-1 text-[13px]" data-testid="run-archive-delta-specs">
      delta specs（{capabilities.length} 个 capability）：{capabilities.join('、')}
    </div>
  );
}

/** 跳过同步勾选（载荷缺席亦呈现——降级不降能）。 */
function ArchiveSyncSkip({
  skipSync,
  onChange,
}: {
  skipSync: boolean;
  onChange: (checked: boolean) => void;
}): React.JSX.Element {
  return (
    <label className="mt-1 flex items-center gap-1.5 text-[13px]">
      <input
        type="checkbox"
        checked={skipSync}
        onChange={(event) => onChange(event.target.checked)}
        data-testid="run-archive-sync-skip"
      />
      跳过 delta specs 同步，直接归档
    </label>
  );
}

/** worktree 合入与清理告知行（目标 = 主仓当前分支；收口后自动移除 worktree、
 * 分支保留——删除磁盘目录属用户可感知动作，确认在先；探测失败呈现占位）。
 * 分支名自归档前置载荷 `branch`（`change/<name>`，worktree 记录在场才在案）。 */
function ArchiveMergeNotice({
  mergeTarget,
  branch,
}: {
  mergeTarget: string | null;
  branch: string | null;
}): React.JSX.Element {
  return (
    <div className="mt-1 break-all text-[13px]" data-testid="run-archive-merge-target">
      将执行提交 + 合入主仓当前分支：
      <span className="text-muted-foreground">{mergeTarget ?? '（当前分支探测失败）'}</span>
      ；收口后自动移除 worktree（分支{' '}
      <span className="text-muted-foreground">{branch ?? 'change/<name>'}</span> 保留）
    </div>
  );
}

/**
 * 架构变更摘要取数（`archi_change_summary` 独立命令，零 `ArchivePreflight` 字段
 * 扩展）：`root` / `changeId` 任一缺席即不发 invoke；`summary === null` = 模型无改
 * 动（区块零占位）。
 */
function useArchiChangeSummary(
  root: string | null,
  changeId: string | null,
): { summary: ArchiChangeSummary | null; error: string | null } {
  const [summary, setSummary] = useState<ArchiChangeSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (root === null || changeId === null) {
      setSummary(null);
      setError(null);
      return;
    }
    let cancelled = false;
    setError(null);
    commands
      .archiChangeSummary(root, changeId)
      .then((result) => {
        if (!cancelled) setSummary(result);
      })
      .catch((reason: unknown) => {
        if (!cancelled) setError(String(reason));
      });
    return () => {
      cancelled = true;
    };
  }, [root, changeId]);
  return { summary, error };
}

/** 计数行（元素 / 关系增删 + gate 变更项数）。 */
function ArchiCountLine({ summary }: { summary: ArchiChangeSummary }): React.JSX.Element {
  return (
    <div className="text-[13px]">
      🏛 架构变更：元素 +{summary.elementsAdded.length} / −{summary.elementsRemoved.length}；关系 +
      {summary.relationsAdded} / −{summary.relationsRemoved}；gate 变更 {summary.gateChanges.length}{' '}
      项
    </div>
  );
}

/** 诊断行（红字，含 `file:line`；**不阻断**确认按钮——结构错由门禁独立处置）。 */
function ArchiDiagnosticRows({
  diagnostics,
}: {
  diagnostics: ArchiChangeSummary['diagnostics'];
}): React.JSX.Element | null {
  if (diagnostics.length === 0) return null;
  return (
    <div className="flex flex-col gap-0.5 text-[13px] text-fail">
      {diagnostics.map((diagnostic, index) => (
        <div
          className="break-all"
          data-testid="run-archi-error"
          key={`${diagnostic.kind}-${diagnostic.file}-${index}`}
        >
          {diagnostic.kind} ·{' '}
          {diagnostic.line === 0 ? diagnostic.file : `${diagnostic.file}:${diagnostic.line}`}：
          {diagnostic.message}
        </div>
      ))}
      <div>修复后再归档（本区块不阻断确认）。</div>
    </div>
  );
}

/** 就地展开的模型 diff 明细（元素增删 FQN 清单 + gate 变更逐条）。 */
function ArchiDiffDetail({ summary }: { summary: ArchiChangeSummary }): React.JSX.Element {
  return (
    <div className="flex flex-col gap-0.5 text-[13px]" data-testid="run-archi-diff-detail">
      {summary.elementsAdded.length > 0 && (
        <div className="break-all">新增元素：{summary.elementsAdded.join('、')}</div>
      )}
      {summary.elementsRemoved.length > 0 && (
        <div className="break-all">删除元素：{summary.elementsRemoved.join('、')}</div>
      )}
      {summary.gateChanges.map((change) => (
        <div className="break-all" key={change.fqn}>
          {change.fqn}：{change.from} → {change.to}
        </div>
      ))}
    </div>
  );
}

/** 新图预览（以改动后的模型渲染画布；worktree 缺席回落主仓根）。 */
function ArchiPreview({ root }: { root: string | null }): React.JSX.Element {
  return (
    <div
      className="h-72 min-h-0 rounded-md border border-border"
      data-testid="run-archi-preview-canvas"
    >
      <ArchitectureCanvas root={root} mode="incremental" />
    </div>
  );
}

/**
 * 归档确认面的「🏛 架构变更」区块（只提高可见性，**不是**新的确认门）：本 change
 * 相对主仓基线的模型结构 diff 摘要，经**独立取数命令** `archi_change_summary`
 * 取得（零 `ArchivePreflight` 字段扩展）。模型无改动 ⇒ `summary === null` ⇒ 区块
 * 整体不渲染（零标题、零空框、零 0 计数行）。模型结构错以红字列出并提示「修复后
 * 再归档」——**不阻断**确认按钮（结构错由门禁与 Preflight 独立处置）。
 */
function ArchitectureChangeBlock({
  root,
  changeId,
  worktree,
}: {
  root: string | null;
  changeId: string | null;
  /** 改后模型的观察根（新图预览用；worktree 缺席时回落主仓根） */
  worktree: string | null;
}): React.JSX.Element | null {
  const { summary, error } = useArchiChangeSummary(root, changeId);
  const [showDiff, setShowDiff] = useState(false);
  const [showPreview, setShowPreview] = useState(false);
  if (error !== null) {
    return (
      <div
        className="mt-1 break-all rounded-md bg-fail-bg px-2.5 py-1.5 text-[13px] text-fail"
        data-testid="run-archi-error"
      >
        架构变更取数失败：{error}
      </div>
    );
  }
  if (summary === null) return null;
  return (
    <div
      className="mt-1 flex flex-col gap-1 rounded-md border border-border bg-background px-2.5 py-1.5"
      data-testid="run-archi-block"
    >
      <ArchiCountLine summary={summary} />
      <ArchiDiagnosticRows diagnostics={summary.diagnostics} />
      <div className="flex gap-2">
        <Button data-testid="run-archi-diff" onClick={() => setShowDiff((open) => !open)}>
          查看模型 diff
        </Button>
        <Button data-testid="run-archi-preview" onClick={() => setShowPreview((open) => !open)}>
          查看新图预览
        </Button>
      </div>
      {showDiff && <ArchiDiffDetail summary={summary} />}
      {showPreview && <ArchiPreview root={worktree ?? root} />}
    </div>
  );
}

/** 归档确认动作行（确认 → proceed=true 且 syncSpecs = `!skipSync`；终止 → 受控
 * 终态 stopped）。 */
function ArchiveConfirmActions({
  actions,
  skipSync,
}: {
  actions: RunActions;
  skipSync: boolean;
}): React.JSX.Element {
  return (
    <div className="mt-2 flex gap-2">
      <Button
        onClick={() => void actions.confirm(true, !skipSync)}
        data-testid="run-archive-confirm-proceed"
      >
        确认归档
      </Button>
      <Button
        onClick={() => void actions.confirm(false, true)}
        data-testid="run-archive-confirm-stop"
      >
        终止运行
      </Button>
    </div>
  );
}

/**
 * 归档前停等确认卡片（stop-before-archive 手动门）：归档前置只读载荷随行
 * （完成度警告 / delta specs 清单 / worktree 合入告知）+ 跳过同步勾选；「确认
 * 归档」进入 archive 相位（proceed=true；syncSpecs = `!skipSync`——勾选跳过即
 * 不回传同步），「终止运行」受控终态 stopped。
 */
function ArchiveConfirmCard({
  actions,
  preflight,
  root,
  changeId,
}: {
  actions: RunActions;
  preflight: ArchivePreflight | null;
  root: string | null;
  changeId: string | null;
}): React.JSX.Element {
  const [skipSync, setSkipSync] = useState(false);
  return (
    <div
      className="mt-2 rounded-md border border-warn bg-card px-3 py-2"
      data-testid="run-archive-confirm-card"
    >
      <div className="text-[13px]">
        等待确认进入归档：确认后进入 archive 相位（提交 worktree / 合入主仓 / 同步 delta specs /
        {preflight !== null && preflight.worktree !== null && ' 自动移除 worktree /'} 双写收口）。
      </div>
      <ArchiveWarningRows warnings={archiveWarnings(preflight)} />
      <ArchiveDeltaSpecs capabilities={preflight?.deltaSpecs ?? []} />
      <ArchitectureChangeBlock
        root={root}
        changeId={changeId}
        worktree={preflight?.worktree ?? null}
      />
      <ArchiveSyncSkip skipSync={skipSync} onChange={setSkipSync} />
      {preflight !== null && preflight.worktree !== null && (
        <ArchiveMergeNotice mergeTarget={preflight.mergeTarget} branch={preflight.branch} />
      )}
      <ArchiveConfirmActions actions={actions} skipSync={skipSync} />
    </div>
  );
}

/** ask 中断卡片：问题 + 选项按钮 + 自由文本应答（应答后 Continue 决策会话）。 */
function AskCard({ actions, ask }: { actions: RunActions; ask: AskPayload }): React.JSX.Element {
  const [text, setText] = useState('');
  const submit = (): void => {
    const trimmed = text.trim();
    if (trimmed === '') return;
    setText('');
    void actions.answer(trimmed);
  };
  return (
    <div
      className="mt-2 rounded-md border border-warn bg-warn-bg px-3 py-2"
      data-testid="run-ask-card"
    >
      <div className="break-words text-[13px]" data-testid="run-ask-question">
        {ask.question}
      </div>
      {ask.options.length > 0 && (
        <div className="mt-1.5 flex flex-wrap gap-1.5">
          {ask.options.map((option) => (
            <Button
              key={option}
              onClick={() => void actions.answer(option)}
              data-testid="run-ask-option"
            >
              {option}
            </Button>
          ))}
        </div>
      )}
      <div className="mt-2 flex gap-2">
        <input
          className="w-full rounded-md border border-border bg-background px-2 py-1 text-[13px] outline-none"
          placeholder="自由应答…"
          value={text}
          onChange={(event) => setText(event.target.value)}
          data-testid="run-ask-input"
        />
        <Button onClick={submit} data-testid="run-ask-submit">
          应答
        </Button>
      </div>
    </div>
  );
}

/** 运行控制面板：状态行 + 停等卡片 + 终态记因。状态徽章自活面（运行中六
 * 值状态机）；activeRun 退场后经 runs 尾行呈现收口终态。 */
/** 状态徽章行：活面六值状态机优先；activeRun 退场后经 runs 尾行呈现收口
 * 终态（unify-run-state-persistence D9）。 */
function RunStatusBadge({
  activeRun,
  lastRun,
}: {
  activeRun: ActiveRunView | null;
  lastRun: ChangeRunEntry | null;
}): React.JSX.Element | null {
  if (activeRun !== null) {
    return (
      <Badge variant={activeRun.status === 'failed' ? 'fail' : 'default'} data-testid="run-status">
        {ACTIVE_STATUS_LABEL[activeRun.status]}
      </Badge>
    );
  }
  if (lastRun !== null) {
    return (
      <Badge variant={lastRun.status === 'failed' ? 'fail' : 'default'} data-testid="run-status">
        {LAST_RUN_STATUS_LABEL[lastRun.status]}
      </Badge>
    );
  }
  return null;
}

export function RunControlPanel({
  // 命令面身份入参保契约面（动作链路已由父层 useChangeFlowRun 按 id 绑定；
  // 面板显名收敛纪律：id 与 name 不静默互换）；架构变更区块以同一 id + 主仓根
  // 独立取数（不经 ArchivePreflight 字段面）
  changeId,
  name,
  root,
  activeRun,
  lastRun,
  actions,
}: RunControlPanelProps): React.JSX.Element {
  const active = activeRun !== null;
  return (
    <section
      className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5"
      data-testid="run-control-panel"
      aria-label={`change ${name} 运行控制`}
    >
      <div className="flex flex-wrap items-center justify-between gap-2">
        <RunStatusBadge activeRun={activeRun} lastRun={lastRun} />
        <RunActions actions={actions} active={active} />
      </div>
      {actions.error !== null && (
        <div
          className="mt-2 break-all rounded-md bg-fail-bg px-2.5 py-1.5 text-[13px] text-fail"
          data-testid="run-error"
        >
          {actions.error}
        </div>
      )}
      {activeRun !== null && activeRun.status === 'waitingConfirm' && activeRun.phase !== null && (
        <ConfirmCard
          actions={actions}
          phase={activeRun.phase}
          preflight={activeRun.archive}
          root={root}
          changeId={changeId}
        />
      )}
      {activeRun !== null && activeRun.ask !== null && (
        <AskCard actions={actions} ask={activeRun.ask} />
      )}
      {activeRun === null && lastRun !== null && lastRun.reason !== null && (
        <div
          className="mt-2 break-words text-[13px] text-muted-foreground"
          data-testid="run-finished-reason"
        >
          收口：{lastRun.reason}
        </div>
      )}
    </section>
  );
}
