import { Button } from '@/components/ui/button';

import type { CodeStatsReport } from '../../types/generated/bindings';
import { DirTree } from './components/dir-tree';
import { LanguageTable } from './components/language-table';
import { StatsSummary } from './components/stats-summary';
import { useCodeStats, type CodeStatsState } from './hooks/use-code-stats';

/** 深度控件值域：1–10（上限收敛树面 DOM 规模；深度不影响汇总 / 语言面数字） */
const DEPTH_OPTIONS = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

/** 页头：标题 + 深度控件（原生 select，变更即重取）+ 刷新钮（loading 期间 disabled） */
function InfoHeader({ state }: { state: CodeStatsState }): React.JSX.Element {
  return (
    <div className="flex flex-wrap items-center justify-between gap-2">
      <h2 className="m-0 text-[15px]">基础信息</h2>
      <div className="flex items-center gap-2">
        <label className="text-xs text-muted-foreground" htmlFor="info-depth">
          解析深度
        </label>
        <select
          id="info-depth"
          className="h-8 rounded-md border border-border bg-background px-2 text-sm"
          data-testid="info-depth-select"
          value={state.depth}
          onChange={(event) => state.setDepth(Number(event.target.value))}
        >
          {DEPTH_OPTIONS.map((option) => (
            <option key={option} value={option}>
              {option}
            </option>
          ))}
        </select>
        <Button disabled={state.loading} data-testid="info-refresh" onClick={state.refresh}>
          刷新
        </Button>
      </div>
    </div>
  );
}

/** 三面呈现区：汇总面 + 语言占比表 + 目录树（树面为空——无目录节点且无文件叶——时省略树区） */
function StatsSections({ report }: { report: CodeStatsReport }): React.JSX.Element {
  return (
    <>
      <section className="rounded-lg border border-border bg-card px-4 py-3.5">
        <h2 className="m-0 mb-3 text-[15px]">汇总</h2>
        <StatsSummary totals={report.totals} />
      </section>
      <section className="rounded-lg border border-border bg-card px-4 py-3.5">
        <h2 className="m-0 mb-3 text-[15px]">语言占比</h2>
        <LanguageTable languages={report.languages} />
      </section>
      {report.tree.length > 0 && (
        <section className="rounded-lg border border-border bg-card px-4 py-3.5">
          <h2 className="m-0 mb-3 text-[15px]">目录分布</h2>
          <DirTree entries={report.tree} />
        </section>
      )}
    </>
  );
}

/**
 * 基础信息页（仅壳态可达，root 由壳态 props 传入）：深度控件 + 显式刷新 +
 * 三面呈现（汇总 / 语言占比 / 目录树）。取数收在 useCodeStats（显式刷新模型，
 * 无轮询无缓存）；失败 inline 持久呈现（查询轨，无 toast 顶替）；解析结果为空
 * 呈现空态而非错误。
 */
export function InfoView({ root }: { root: string }): React.JSX.Element {
  const state = useCodeStats(root);
  const empty = state.report !== null && state.report.totals.files === 0;
  return (
    <div className="flex flex-col gap-3" data-testid="info-view">
      <InfoHeader state={state} />
      {state.error !== null && (
        <div
          className="break-all rounded-md bg-fail-bg px-3 py-2 text-fail"
          data-testid="info-error"
        >
          解析失败：{state.error}
        </div>
      )}
      {state.loading && (
        <div className="text-muted-foreground" data-testid="info-loading">
          解析中…
        </div>
      )}
      {!state.loading && empty && (
        <div className="text-muted-foreground" data-testid="info-empty">
          当前工作区未识别到代码文件。
        </div>
      )}
      {!state.loading && !empty && state.report !== null && <StatsSections report={state.report} />}
    </div>
  );
}
