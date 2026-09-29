import { Progress } from '@/components/ui/progress';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';

import type { LanguageStats } from '../../../types/generated/bindings';

/** 占比单元格：Progress 承载份额（value 直传百分点，组件内部以 transform 实现），旁注百分比数值 */
function ShareCell({ share }: { share: number | null }): React.JSX.Element {
  const value = share ?? 0;
  return (
    <span className="flex items-center gap-2">
      <Progress className="w-24" value={value} />
      <span className="whitespace-nowrap text-xs tabular-nums text-muted-foreground">
        {value.toFixed(1)}%
      </span>
    </span>
  );
}

/**
 * 语言占比表：语言 | 文件数 | 代码 | 注释 | 空行 | 占比。行序即入参序
 * （后端已按代码行降序、tie 语言名字典序排序），本组件不重排。
 */
export function LanguageTable({ languages }: { languages: LanguageStats[] }): React.JSX.Element {
  return (
    <Table data-testid="info-language-table">
      <TableHeader>
        <TableRow>
          <TableHead>语言</TableHead>
          <TableHead>文件数</TableHead>
          <TableHead>代码</TableHead>
          <TableHead>注释</TableHead>
          <TableHead>空行</TableHead>
          <TableHead>占比</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        {languages.map((language) => (
          <TableRow data-lang={language.name} data-testid="info-language-row" key={language.name}>
            <TableCell>{language.name}</TableCell>
            <TableCell>{language.files.toLocaleString()}</TableCell>
            <TableCell>{language.code.toLocaleString()}</TableCell>
            <TableCell>{language.comments.toLocaleString()}</TableCell>
            <TableCell>{language.blanks.toLocaleString()}</TableCell>
            <TableCell>
              <ShareCell share={language.share} />
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}
