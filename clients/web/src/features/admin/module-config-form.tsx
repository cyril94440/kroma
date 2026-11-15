import { useWebRuntime } from '#web/shared/lib/web-runtime';
// Typed settings form for a module's declared `config` fields. Renders the right
// control per field kind (text / number / checkbox / select) and PUTs properly
// typed JSON values to /api/admin/modules/:id/config (bool and number, not the
// stringified values the old text-only form sent).

import type { ConfigField } from '@kromatv/module-sdk';
import { useT } from '@kromatv/ui';
import {
  Box,
  Button,
  Divider,
  Field,
  ListRow,
  NumberField,
  Row,
  Select,
  Switch,
  styles,
  useStyles,
} from '@kromatv/ui/kit';
import { type ReactNode, useState } from 'react';
import { type ConfigLocales, useConfigText } from './module-config-text';

type ConfigValue = string | number | boolean;

const CONTROL_WIDTH = 160;
const sDefinition = styles({ select: { width: CONTROL_WIDTH } });

const KEPT_SECRET = '\u2022\u2022\u2022\u2022\u2022\u2022\u2022\u2022';

function hasStoredSecret(field: ConfigField, stored: unknown): boolean {
  return field.secret === true && stored === true;
}

function initial(field: ConfigField, stored: unknown): ConfigValue {
  if (field.secret) return '';
  const raw = stored ?? field.default;
  switch (field.type) {
    case 'bool':
      return raw === true || raw === 'true';
    case 'number': {
      const n = typeof raw === 'number' ? raw : Number(raw);
      return Number.isFinite(n) ? n : 0;
    }
    default:
      if (raw == null) return '';
      if (typeof raw === 'string') return raw;
      if (typeof raw === 'number' || typeof raw === 'boolean') return String(raw);
      return JSON.stringify(raw);
  }
}

export function ModuleConfigForm({
  moduleId,
  fields,
  values,
  locales,
  onSaved,
}: Readonly<{
  moduleId: string;
  fields: ConfigField[];
  values: Record<string, unknown>;
  locales?: ConfigLocales;
  onSaved: () => void;
}>) {
  const webRuntime = useWebRuntime();

  const t = useT();
  const words = useConfigText(locales);
  const [draft, setDraft] = useState<Record<string, ConfigValue>>(() =>
    Object.fromEntries(fields.map((f) => [f.key, initial(f, values[f.key])])),
  );
  const [saving, setSaving] = useState(false);
  const [cleared, setCleared] = useState<ReadonlySet<string>>(new Set());
  const set = (key: string, v: ConfigValue) => {
    setDraft((d) => ({ ...d, [key]: v }));
    if (v !== '') setCleared((c) => new Set([...c].filter((k) => k !== key)));
  };
  const clear = (key: string) => {
    setDraft((d) => ({ ...d, [key]: '' }));
    setCleared((c) => new Set(c).add(key));
  };

  const save = async () => {
    setSaving(true);
    try {
      await webRuntime.moduleApi.adminApi(`/modules/${encodeURIComponent(moduleId)}/config`, {
        method: 'PUT',
        body: JSON.stringify(
          Object.fromEntries(
            Object.entries(draft).flatMap(([key, value]): [string, ConfigValue | null][] => {
              if (!fields.find((f) => f.key === key)?.secret) return [[key, value]];
              if (value !== '') return [[key, value]];
              return cleared.has(key) ? [[key, null]] : [];
            }),
          ),
        ),
      });
      onSaved();
    } finally {
      setSaving(false);
    }
  };

  return (
    <>
      <Box mt={12}>
        <Divider />
      </Box>
      <Box gap={8} pt={12}>
        <ListRow.Group size="sm">
          {fields.map((f) => (
            <ConfigRow
              key={f.key}
              field={f}
              value={draft[f.key]}
              keeps={hasStoredSecret(f, values[f.key]) && !cleared.has(f.key)}
              words={words}
              onChange={(v) => set(f.key, v)}
              onClear={() => clear(f.key)}
            />
          ))}
        </ListRow.Group>
        <Row justify="flex-end">
          <Button
            variant="outline"
            active
            size="sm"
            label={t('common.save')}
            onPress={() => void save()}
            loading={saving}
          />
        </Row>
      </Box>
    </>
  );
}

function ConfigRow({
  field,
  value,
  keeps,
  words,
  onChange,
  onClear,
}: Readonly<{
  field: ConfigField;
  value: ConfigValue | undefined;
  keeps: boolean;
  words: (text: string) => string;
  onChange: (v: ConfigValue) => void;
  onClear: () => void;
}>) {
  const t = useT();
  const s = useStyles(sDefinition);
  const label = words(field.label);
  const hint = field.placeholder ? words(field.placeholder) : undefined;
  const placeholder = keeps ? KEPT_SECRET : hint;

  let control: ReactNode;
  if (field.type === 'bool') {
    control = <Switch checked={value === true} onCheckedChange={onChange} label={label} />;
  } else if (field.type === 'select') {
    control = (
      <Select.Root label={label} value={String(value ?? '')} onValueChange={onChange}>
        <Select.Trigger style={s.select} />
        {(field.options ?? []).map((opt) => (
          <Select.Item key={opt} value={opt} label={words(opt)} />
        ))}
      </Select.Root>
    );
  } else if (field.type === 'number') {
    control = (
      <NumberField
        label={label}
        value={typeof value === 'number' ? value : 0}
        onValueChange={onChange}
        w={CONTROL_WIDTH}
      />
    );
  } else {
    control = (
      <Field.Root label={label} hideLabel w={CONTROL_WIDTH}>
        <Field.Input
          type={field.secret ? 'password' : 'text'}
          placeholder={placeholder}
          value={String(value ?? '')}
          onValueChange={onChange}
        />
      </Field.Root>
    );
  }

  return (
    <ListRow.Root size="sm">
      <ListRow.Label>{label}</ListRow.Label>
      <ListRow.Trailing>
        {keeps ? (
          <Row gap={8} align="center">
            {control}
            <Button variant="ghost" size="sm" label={t('common.clear')} onPress={onClear} />
          </Row>
        ) : (
          control
        )}
      </ListRow.Trailing>
    </ListRow.Root>
  );
}
