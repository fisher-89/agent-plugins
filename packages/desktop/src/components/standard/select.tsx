import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';

interface StandardSelectProps<T extends string | number> {
  value: T | null;
  onChange: (value: T | null) => void;
  items: { value: string | number; label: string }[];
  placeholder?: string;
  disabled?: boolean;
  'data-testid'?: string;
  id?: string;
  className?: string;
}

export function StandardSelect<T extends string | number>({
  value,
  onChange,
  items,
  placeholder = '请选择',
  disabled = false,
  'data-testid': testId,
  ...rest
}: StandardSelectProps<T>) {
  return (
    <Select value={value} items={items} disabled={disabled} onValueChange={onChange}>
      <SelectTrigger>
        <SelectValue data-testid={testId} data-value={value} placeholder={placeholder} {...rest} />
      </SelectTrigger>
      <SelectContent>
        {items.map((item) => (
          <SelectItem key={item.value} value={item.value}>
            {item.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
