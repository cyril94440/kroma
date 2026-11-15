// @vitest-environment jsdom
import { I18nProvider } from '@kromatv/ui';
import { createInputTestHost } from '@kromatv/ui/testing';
import { cleanup, fireEvent, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { ModuleConfigForm } from '#web/features/admin/module-config-form';

const H = vi.hoisted(() => ({ adminApi: vi.fn() }));

vi.mock('#web/shared/lib/web-runtime', () => ({
  useWebRuntime: () => ({ moduleApi: { adminApi: H.adminApi } }),
}));

const { render } = createInputTestHost();

afterEach(cleanup);

const LOCALES = {
  en: { 'config.cacheMaxGb': 'Cache size limit in GB (0 = no cache)', 'config.mode': 'Mode' },
  fr: { 'config.cacheMaxGb': 'Taille maximale du cache en Go (0 = pas de cache)' },
};

function show(locale: 'en' | 'fr', fields: Parameters<typeof ModuleConfigForm>[0]['fields']) {
  render(
    <I18nProvider locale={locale}>
      <ModuleConfigForm
        moduleId="tv.kroma.engine.alldebrid"
        fields={fields}
        values={{}}
        locales={LOCALES}
        onSaved={() => {}}
      />
    </I18nProvider>,
  );
}

const CACHE_GB = {
  key: 'cacheMaxGb',
  label: 'config.cacheMaxGb',
  type: 'number',
  default: '50',
} as const;

it("words a field's label from the module's own catalog in the reader's language", () => {
  show('fr', [CACHE_GB]);

  expect(
    screen.getAllByText('Taille maximale du cache en Go (0 = pas de cache)').length,
  ).toBeGreaterThan(0);
});

it('falls back to the module English catalog when the reader language has no entry', () => {
  show('fr', [{ key: 'mode', label: 'config.mode', type: 'string' }]);

  expect(screen.getAllByText('Mode').length).toBeGreaterThan(0);
});

it('falls back to the app catalog for a core key', () => {
  show('en', [{ key: 'password', label: 'field.apiKey', type: 'string', secret: true }]);

  expect(screen.getAllByText('API key').length).toBeGreaterThan(0);
});

it('renders a label no catalog knows exactly as the module wrote it', () => {
  show('en', [{ key: 'region', label: 'Preferred region', type: 'string' }]);

  expect(screen.getAllByText('Preferred region').length).toBeGreaterThan(0);
});

it('never renders a stored secret back, and an untouched secret is left out of the save', async () => {
  const fields = [
    { key: 'primaryUrl', label: 'Portal', type: 'string', secret: true },
    { key: 'agent', label: 'Agent', type: 'string' },
  ] as const;
  const { container } = render(
    <I18nProvider locale="en">
      <ModuleConfigForm
        moduleId="tv.kroma.live"
        fields={[...fields]}
        values={{ primaryUrl: 'http://portal/get.php?password=hunter2', agent: 'VLC' }}
        onSaved={() => {}}
      />
    </I18nProvider>,
  );

  expect(container.innerHTML).not.toContain('hunter2');

  fireEvent.click(screen.getByText('Save'));
  await Promise.resolve();

  const body = JSON.parse(H.adminApi.mock.calls.at(-1)?.[1].body as string);
  expect(body).toEqual({ agent: 'VLC' });
});
