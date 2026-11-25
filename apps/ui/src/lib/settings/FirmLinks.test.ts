import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AppSettings } from '$api/AppSettings';
import type { FirmLink } from '$api/FirmLink';
import type { SettingsPatch } from '$api/SettingsPatch';
import { toast } from '#lib/ui/toast/toast.svelte.js';
import FirmLinks from './FirmLinksHarness.test-helper.svelte';
import { createSettingsEditor, type SettingsEditor } from './settings-editor.svelte.js';

const LINKS: FirmLink[] = [
	{ id: 'telegram', name: 'Telegram', url: 'https://t.example/x', enabled: true },
	{ id: 'link_site', name: 'Сайт', url: 'https://site.example', enabled: true },
	{ id: 'link_vk', name: 'ВК', url: 'https://vk.example', enabled: true }
];

// The server's settings: a save becomes them, as after the answer and `refreshCatalogs`.
const app = vi.hoisted(() => ({
	settings: { firm_links: [] as FirmLink[] },
	can: () => true
}));
vi.mock('#lib/app/context.svelte.js', () => ({ app }));

let editor: SettingsEditor;
let save: ReturnType<typeof vi.fn<(patch: SettingsPatch) => Promise<AppSettings>>>;

function setup() {
	app.settings.firm_links = LINKS.map((l) => ({ ...l }));
	save = vi.fn(async (patch: SettingsPatch) => {
		if (patch.firm_links) app.settings.firm_links = patch.firm_links;
		return app.settings as AppSettings;
	});
	editor = createSettingsEditor({ can: () => true, save, refresh: async () => {} });
	return render(FirmLinks, { editor });
}

const names = () => screen.getAllByRole('group').map((g) => g.getAttribute('aria-label'));
const lastSaved = () => save.mock.calls.at(-1)?.[0].firm_links?.map((l) => l.id);

afterEach(() => {
	editor.dispose();
	toast.clear();
});

describe('FirmLinks', () => {
	it('an own link deleted by mistake comes back to its place with its id by «Вернуть» of the toast', async () => {
		setup();
		const site = screen.getByRole('group', { name: 'Сайт' });
		await userEvent.click(within(site).getByRole('button', { name: 'Удалить ссылку' }));
		await waitFor(() => expect(lastSaved()).toEqual(['telegram', 'link_vk']));
		expect(names()).toEqual(['Telegram', 'ВК']);
		expect(await screen.findByText('Ссылка «Сайт» удалена')).toBeInTheDocument();

		await userEvent.click(screen.getByRole('button', { name: 'Вернуть' }));
		await waitFor(() => expect(lastSaved()).toEqual(['telegram', 'link_site', 'link_vk']));
		expect(save.mock.calls.at(-1)?.[0].firm_links?.[1]).toEqual(LINKS[1]);
		expect(names()).toEqual(['Telegram', 'Сайт', 'ВК']);
		expect(screen.queryByText('Ссылка «Сайт» удалена')).toBeNull();
	});

	it('leaving the list takes its toast away: «Вернуть» never writes from a closed page', async () => {
		const { unmount } = setup();
		const vk = screen.getByRole('group', { name: 'ВК' });
		await userEvent.click(within(vk).getByRole('button', { name: 'Удалить ссылку' }));
		await screen.findByText('Ссылка «ВК» удалена');
		unmount();
		expect(toast.items).toEqual([]);
	});
});
