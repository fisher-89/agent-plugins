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
 * 「目录取数失败即无建议」的降级落点。选中值不受控也不读取：唯一真相是输入
 * 文本（双真相会让底座在 blur / 重开时把用户键入吞掉）；弹层开合由底座管理，
 * 本层零第二份开关状态。
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
    <Combobox inputValue={value} onInputValueChange={(next) => onChange(next)} disabled={disabled}>
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
