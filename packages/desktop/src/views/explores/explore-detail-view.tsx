import { useEffect, useRef, useState } from 'react';
import { useNavigate, useParams } from 'react-router';

import type { ExploreRecord } from '../../types/dto';
import { ExploreDetailPane } from './explore-detail-pane';
import { useExploreRouteContext } from './explore-layout';

/**
 * 探索详情页（路由 `/explores/:name`）：选中由路由参数承载（无本地 state 双轨），
 * 记录从布局清单三级派生——URL 名 find → miss 时以 `lastRecordRef` 快照 id 在
 * 清单重寻（id 桥：in-place 改名保 id，旧名寻址不再命中）→ 仍 miss 以快照本体
 * 兜底（MUST NOT 闪现 MissingRecord）。workspace 根切换（select / 移除当前根 /
 * 添加新根）时带旧选中即 replace 落清单。渲染与动作面见 [`ExploreDetailPane`]。
 */
export function ExploreDetailView({ root }: { root: string }): React.JSX.Element {
  const { name } = useParams<'name'>();
  const { list } = useExploreRouteContext();
  const navigate = useNavigate();
  const [prevRoot, setPrevRoot] = useState(root);
  const [resetPending, setResetPending] = useState(false);
  const lastRecordRef = useRef<ExploreRecord | null>(null);

  // 早于 navigate 过渡提交，否则留出「新根 + 旧名」中间提交重新武装查询
  if (prevRoot !== root) {
    setPrevRoot(root);
    if (name !== undefined) setResetPending(true);
  }
  if (resetPending && name === undefined) {
    setResetPending(false);
  }

  useEffect(() => {
    if (resetPending) {
      void navigate('/explores', { replace: true }); // workspace 切换落清单
    }
  }, [resetPending, navigate]);

  let record: ExploreRecord | null = null;
  if (name !== undefined && !resetPending) {
    const byName = list.records.find((item) => item.name === name);
    if (byName !== undefined) {
      record = byName;
    } else if (lastRecordRef.current !== null) {
      const snapshot = lastRecordRef.current;
      record = list.records.find((item) => item.id === snapshot.id) ?? snapshot;
    }
  }
  if (record !== null) {
    lastRecordRef.current = record; // 派生非空时渲染期刷新快照
  }

  return <ExploreDetailPane list={list} record={record} root={root} />;
}
