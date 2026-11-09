import { useEffect, useRef, useState } from 'react';
import { useNavigate, useParams } from 'react-router';

import type { ExploreRecord } from '@/types/dto';

import { ExploreDetailPane } from './explore-detail-pane';
import { useExploreRouteContext } from './explore-layout';

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
