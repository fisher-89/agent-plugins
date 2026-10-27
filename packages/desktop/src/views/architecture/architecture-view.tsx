/**
 * 架构页（挂载点 A）：观察根 = 壳态当前根（URL 不携根段），`mode = "full"` 全量
 * 只读——不呈现任何可提交的改档入口。
 *
 * 取数**显式**：刷新按钮以 `key` 重挂画布（挂载即取数），零 watch 零轮询；磁盘上
 * 改动模型后须用户点刷新才可见。
 */
import { useState } from 'react';

import { Button } from '@/components/ui/button';

import { ArchitectureCanvas } from './components/architecture-canvas';

export function ArchitectureView({ root }: { root: string | null }): React.JSX.Element {
  const [refreshTick, setRefreshTick] = useState(0);
  return (
    <div className="flex h-full min-h-0 flex-col gap-2">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <h2 className="m-0 text-[15px]">架构</h2>
        <Button data-testid="archi-refresh" onClick={() => setRefreshTick((tick) => tick + 1)}>
          刷新
        </Button>
      </div>
      <div className="min-h-0 flex-1">
        <ArchitectureCanvas key={refreshTick} root={root} mode="full" />
      </div>
    </div>
  );
}
