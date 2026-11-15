import { Chip, FocusRegion, styles, useStyles } from '@kromatv/ui/kit';

const sDefinition = styles({ row: { flexDirection: 'row', flexWrap: 'wrap', gap: 10 } });

export interface Tab<T extends string> {
  id: T;
  label: string;
}

/** The sections of a drawer as a row the remote walks left and right and
 *  leaves downwards; a radio group would take Up and Down as well and keep the
 *  focus on its own segments. The tab under the remote opens on press. */
export function TabStrip<T extends string>({
  tabs,
  value,
  onChange,
}: Readonly<{ tabs: readonly Tab<T>[]; value: T; onChange: (next: T) => void }>) {
  const s = useStyles(sDefinition);
  return (
    <FocusRegion style={s.row}>
      {tabs.map((tab) => (
        <Chip
          key={tab.id}
          label={tab.label}
          active={tab.id === value}
          onPress={() => onChange(tab.id)}
        />
      ))}
    </FocusRegion>
  );
}
