import { Button } from '@/components/ui/button';

import type { WorkspaceState } from '../hooks/useWorkspaces';

/**
 * 欢迎屏：无 workspace 记录时的空态 +「添加新文件夹」入口
 * + 加载中 / error-note 呈现（error-note 仅承载清单加载失败；添加等动作失败
 * 经 toast 呈现）。（清单非空时启动即恢复第一名，不停留此处。）
 */
export function WelcomeView({ state, onAdd }: { state: WorkspaceState; onAdd: () => void }) {
  const { workspaces, loading, error } = state;
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-4">
      <h1 className="m-0 text-xl">Dev Team</h1>
      <p className="m-0 text-muted-foreground">选择一个项目根目录，浏览其 change 过程记录。</p>
      <div className="mx-auto w-full max-w-[1100px] flex-1 px-4 py-4">
        {error !== null && (
          <div
            className="mb-3 break-all rounded-md bg-fail-bg px-3 py-2 text-fail"
            data-testid="error-note"
          >
            workspace 清单加载失败：{error}
          </div>
        )}
        {loading && <div className="text-muted-foreground">加载中…</div>}
        <section className="mb-4 rounded-lg border border-border bg-card px-4 py-3.5">
          <h2 className="m-0 mb-2.5 text-[15px]">
            最近的 workspace <span className="text-muted-foreground">({workspaces.length})</span>
          </h2>
          {!loading && error === null && workspaces.length === 0 && (
            <div className="text-muted-foreground">还没有记录，添加一个项目根目录开始浏览。</div>
          )}
        </section>
        <Button onClick={onAdd}>添加新文件夹</Button>
      </div>
    </div>
  );
}
