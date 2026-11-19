import { Combobox, ComboboxContent, ComboboxInput, ComboboxItem } from '@/components/ui/combobox';

interface StandardComboboxProps {
  value: string;
  onChange: (value: string) => void;
  items: string[];
  placeholder?: string;
  disabled?: boolean;
  'data-testid'?: string;
  id?: string;
  className?: string;
}

/**
 * 「可选可填」单值控件：输入框可自由键入（建议不构成约束），下拉给出建议清单。
 *
 * 值语义：空串 = 留空（与 `StandardSelect` 的 `null` 表未选择分开）；清空输入即
 * 回调空串。`items` 为空数组时退化为纯输入框（不渲染弹层、不报错）——这是
 * 「目录取数失败即无建议」的降级落点。唯一真相是输入文本：选中值也受控为同一
 * 原文（否则底座会在关层收尾时按「未选中 = 空」回写、把用户键入吞掉）；弹层
 * 开合由底座管理，本层零第二份开关状态。
 */
export function StandardCombobox({
  value,
  onChange,
  items,
  placeholder = '可输入或选择',
  disabled = false,
  'data-testid': testId,
  id,
  className,
}: StandardComboboxProps) {
  return (
    <Combobox
      // 选中值同步受控 = 输入原文：base-ui 单值模式在弹层关闭收尾时把输入回写
      // 「已选中值标签」（未选中过则为空串），不受控会让自由键入随关层被清空；
      // 两路同源后回写恒等于现值（选中条目仍经 onValueChange → 回填同一原文）
      inputValue={value}
      onInputValueChange={(next) => onChange(next)}
      onValueChange={(next) => onChange(next ?? '')}
      value={value}
      disabled={disabled}
    >
      <ComboboxInput
        id={id}
        className={className}
        placeholder={placeholder}
        data-testid={testId}
        data-value={value}
      />
      {items.length > 0 && (
        <ComboboxContent>
          {items.map((item) => (
            <ComboboxItem key={item} value={item}>
              {item}
            </ComboboxItem>
          ))}
        </ComboboxContent>
      )}
    </Combobox>
  );
}
