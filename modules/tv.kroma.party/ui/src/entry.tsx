import { Field } from '@kromatv/ui/kit';

interface EntryProps {
  label: string;
  value: string;
  onValueChange: (next: string) => void;
  onSubmit?: () => void;
  placeholder?: string;
  search?: boolean;
}

export function Entry({
  label,
  value,
  onValueChange,
  onSubmit,
  placeholder,
  search,
}: Readonly<EntryProps>) {
  return (
    <Field.Root label={label} hideLabel value={value} onValueChange={onValueChange}>
      <Field.Input
        onSubmit={onSubmit}
        placeholder={placeholder}
        type={search ? 'search' : 'text'}
        icon={search ? 'search' : undefined}
      />
    </Field.Root>
  );
}
