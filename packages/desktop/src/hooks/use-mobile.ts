import { useEffect, useState } from 'react';

const MOBILE_BREAKPOINT = 768;

/** 断点 hook：窗口宽 < 768px → true（sidebar 抽屉第三态分流的唯一判定来源）。
 * 挂载后 matchMedia 首测 + change 监听续测；首帧返回 false（desktop 形态）。 */
export function useIsMobile(): boolean {
  const [isMobile, setIsMobile] = useState<boolean | undefined>(undefined);

  useEffect(() => {
    const mql = window.matchMedia(`(max-width: ${MOBILE_BREAKPOINT - 1}px)`);
    const onChange = () => {
      setIsMobile(window.innerWidth < MOBILE_BREAKPOINT);
    };
    mql.addEventListener('change', onChange);
    setIsMobile(window.innerWidth < MOBILE_BREAKPOINT);
    return () => mql.removeEventListener('change', onChange);
  }, []);

  return isMobile ?? false;
}
