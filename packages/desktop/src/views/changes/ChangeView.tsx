import { useCallback, useEffect, useState } from 'react';
import { useNavigate, useParams } from 'react-router';

import { type ChangeListState } from '../../hooks/useChangeList';
import { ChangeDetailView } from './ChangeDetailView';
import { ChangeListView } from './ChangeListView';
import { useChangeDetail } from './hooks/useChangeDetail';

/**
 * 变更视图：/changes（清单）与 /changes/:name（详情）共用；选中 change 由
 * 路由参数承载（useParams 派生，无本地 state 双轨），行点击 / 返回均显式
 * navigate。workspace 根切换（select / 移除当前根 / 添加新根）时若带旧选中，
 * 过渡轮先以 null 取数（抑制「新根 + 旧名」误发 get_change_detail），effect
 * 中 replace 导航回 /changes（URL 无 :name 段）后清位。
 */
export function ChangeView({ root, list }: { root: string | null; list: ChangeListState }) {
  const { name } = useParams<'name'>();
  const navigate = useNavigate();
  const selected = name ?? null;
  const [prevRoot, setPrevRoot] = useState(root);
  const [resetPending, setResetPending] = useState(false);
  const detail = useChangeDetail(root, resetPending ? null : selected);

  // 渲染期调整（沿用既有模式）：根切换且带旧选中 → 置待导航标记；抑制位在
  // URL 已失去 :name 段（navigate 过渡提交落地）后清位。清位不可早于落点
  // 提交：navigate 经 transition 提交，与同步清位不同 commit，先清位会留出
  // 一个「新根 + 旧名」中间提交，重新武装 get_change_detail 误发（AC-7）。
  if (prevRoot !== root) {
    setPrevRoot(root);
    if (selected !== null) setResetPending(true);
  }
  if (resetPending && selected === null) {
    setResetPending(false);
  }

  useEffect(() => {
    if (resetPending) {
      void navigate('/changes', { replace: true }); // workspace 切换落清单，URL 无 :name 段
    }
  }, [resetPending, navigate]);

  const openChange = useCallback((n: string) => navigate(`/changes/${n}`), [navigate]);
  const backToList = useCallback(() => navigate('/changes'), [navigate]); // 显式返回，不用 navigate(-1)

  return selected === null ? (
    <ChangeListView state={list} onSelect={openChange} />
  ) : (
    <ChangeDetailView state={detail} onBack={backToList} />
  );
}
