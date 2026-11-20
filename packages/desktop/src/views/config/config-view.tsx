import { Button } from '@/components/ui/button';
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable';

import type { WorkspaceConfigReport } from '../../types/generated/bindings';
import { BasicConfigSection } from './components/basic-config-section';
import { ConfigChatPanel } from './components/config-chat-panel';
import { DiagnosticsSection } from './components/diagnostics-section';
import { ExtraFieldsSection } from './components/extra-fields-section';
import { TestsSection } from './components/tests-section';
import { WriteProtectionSection } from './components/write-protection-section';
import { useWorkspaceConfig, type WorkspaceConfigState } from './hooks/use-workspace-config';

/** 页头：标题 + 刷新钮（loading 期间 disabled；写回的唯一入口在左栏提案卡上） */
function ConfigHeader({ state }: { state: WorkspaceConfigState }): React.JSX.Element {
  return (
    <div className="flex flex-wrap items-center justify-between gap-2">
      <h2 className="m-0 text-[15px]">工作区配置</h2>
      <Button disabled={state.loading} data-testid="config-refresh" onClick={state.refresh}>
        刷新
      </Button>
    </div>
  );
}

/** 诊断派生面：吃默认字段 path 集合（分区「未设（默认 N）」标注的数据来源） */
function deriveDefaultedPaths(report: WorkspaceConfigReport | null): ReadonlySet<string> {
  const paths = new Set<string>();
  for (const diagnostic of report?.diagnostics ?? []) {
    if (diagnostic.kind === 'defaultApplied') {
      paths.add(diagnostic.path);
    }
  }
  return paths;
}

/** 文件级 fatal 诊断（报告内错误来源，与命令 reject 并列双来源） */
function hasFatalDiagnostic(report: WorkspaceConfigReport | null): boolean {
  return (
    report?.diagnostics.some(
      (diagnostic) => diagnostic.kind === 'readFailed' || diagnostic.kind === 'jsonInvalid',
    ) ?? false
  );
}

/** 文件缺失标记（空态来源，非错误） */
function isFileMissing(report: WorkspaceConfigReport | null): boolean {
  return report?.diagnostics.some((diagnostic) => diagnostic.kind === 'fileMissing') ?? false;
}

/** 状态面：inline 持久错误（命令 reject 与文件级 fatal 双来源，无 toast 顶替）/
 * loading 行 / 空态（文件缺失，文案说明默认值行为） */
function ConfigStatusFaces({
  state,
  fatal,
  missing,
}: {
  state: WorkspaceConfigState;
  fatal: boolean;
  missing: boolean;
}): React.JSX.Element {
  return (
    <>
      {(state.error !== null || fatal) && (
        <div
          className="break-all rounded-md bg-fail-bg px-3 py-2 text-fail"
          data-testid="config-error"
        >
          {state.error !== null
            ? `解析失败：${state.error}`
            : '配置文件读取或解析失败，以下为默认值。'}
        </div>
      )}
      {state.loading && (
        <div className="text-muted-foreground" data-testid="config-loading">
          解析中…
        </div>
      )}
      {!state.loading && !fatal && state.error === null && missing && (
        <div className="text-muted-foreground" data-testid="config-empty">
          当前工作区无配置文件，以下呈现全部默认值。
        </div>
      )}
    </>
  );
}

/**
 * 工作区配置页（仅壳态可达，root 由壳态 props 传入）：左栏常驻 config 会话面板
 * （骨架自带折叠，缺省收起）+ 右栏页头（标题 + 刷新钮）、状态面与五分区编排
 * （diagnostics 警示区置顶 → 基础配置 → tests → 写入保护 → 未知字段）。
 *
 * 呈现面零编辑控件：写回的唯一入口是左栏提案卡的「应用」（`onApplied` 触发本页
 * 唯一的取数路径 `useWorkspaceConfig().refresh()`）；取数收在 `useWorkspaceConfig`
 * （显式刷新模型，无轮询无缓存）。
 */
export function ConfigView({ root }: { root: string }): React.JSX.Element {
  const state = useWorkspaceConfig(root);
  const fatal = hasFatalDiagnostic(state.data);
  const missing = isFileMissing(state.data);
  const defaultedPaths = deriveDefaultedPaths(state.data);
  const showSections =
    !state.loading && state.error === null && !fatal && !missing && state.data !== null;
  return (
    <div className="flex h-full min-h-0 flex-col gap-2" data-testid="config-view">
      <ResizablePanelGroup className="min-h-0 flex-1" orientation="horizontal">
        <ConfigChatPanel root={root} onApplied={state.refresh} />
        <ResizableHandle withHandle />
        <ResizablePanel minSize="40">
          <div className="flex h-full min-h-0 flex-col gap-3 overflow-auto pl-1.5">
            <ConfigHeader state={state} />
            <ConfigStatusFaces fatal={fatal} missing={missing} state={state} />
            {showSections && state.data !== null && (
              <>
                <DiagnosticsSection diagnostics={state.data.diagnostics} />
                <BasicConfigSection config={state.data.config} defaultedPaths={defaultedPaths} />
                <TestsSection defaultedPaths={defaultedPaths} suites={state.data.config.tests} />
                <WriteProtectionSection protection={state.data.config.writeProtection} />
                <ExtraFieldsSection extra={state.data.config.extra} />
              </>
            )}
          </div>
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  );
}
