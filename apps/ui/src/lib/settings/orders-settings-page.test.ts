import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { ru } from '#lib/i18n/ru.js';

// «Настройки → Заказы»: «Гарантия по умолчанию» is a list of the dictionary of warranties, with «Не задана» first;
// a saved value that the dictionary does not have stays shown and chosen.
const api = vi.hoisted(() => ({ listDictionary: vi.fn(), updateSettings: vi.fn() }));
vi.mock('#lib/api/catalogs.js', () => api);
const masters = vi.hoisted(() => ({ listMasters: vi.fn() }));
vi.mock('#lib/api/masters.js', () => masters);
const state = vi.hoisted(() => ({
	settings: {
		default_service_point_id: null,
		default_master_user_id: null,
		default_warranty: '30 дней',
		diagnostics_overdue_days: 4,
		ready_stale_days: 7,
		intake_required_fields: false,
		issue_required: []
	} as Record<string, unknown>,
	refreshCatalogs: vi.fn(async () => {})
}));
vi.mock('#lib/app/context.svelte.js', () => ({
	app: {
		can: () => true,
		get settings() {
			return state.settings;
		},
		activePoints: [],
		refreshCatalogs: state.refreshCatalogs
	}
}));

import Page from './OrdersSettingsHarness.test-helper.svelte';

// What bits-ui calls on a pointer press and jsdom does not have.
beforeAll(() => {
	Element.prototype.hasPointerCapture ??= () => false;
	Element.prototype.releasePointerCapture ??= () => {};
	Element.prototype.scrollIntoView ??= () => {};
});

const item = (id: number, value: string, archived = false) => ({
	id,
	kind: 'warranty',
	value,
	sort_order: id,
	archived
});

beforeEach(() => {
	api.listDictionary.mockReset();
	api.updateSettings.mockReset();
	api.listDictionary.mockImplementation(async (kind: string) =>
		kind === 'warranty' ? [item(1, '14 дней'), item(2, '30 дней'), item(3, '1 год', true)] : []
	);
	api.updateSettings.mockImplementation(async (patch: Record<string, unknown>) => ({
		...state.settings,
		...patch
	}));
	masters.listMasters.mockReset();
	masters.listMasters.mockResolvedValue([
		{ id: 3, display_name: 'Антон', active: true },
		{ id: 7, display_name: 'Уволен', active: false }
	]);
	state.settings = { ...state.settings, default_warranty: '30 дней', default_master_user_id: null };
});

describe('«Мастер по умолчанию»', () => {
	it('offers «Не выбран» and the active masters; the choice saves the user', async () => {
		render(Page);
		await waitFor(() => expect(masters.listMasters).toHaveBeenCalled());
		const box = screen.getByRole('combobox', { name: 'Мастер по умолчанию' });
		expect(box).toHaveTextContent('Не выбран');
		await waitFor(async () => {
			box.focus();
			await userEvent.keyboard('{Enter}');
			await screen.findAllByRole('option', { hidden: true });
			expect(optionTexts()).toEqual(['Не выбран', 'Антон']);
		});
		await userEvent.click(option('Антон'));
		await waitFor(() =>
			expect(api.updateSettings).toHaveBeenCalledWith({ default_master_user_id: 3 })
		);
	});
});

/** The list is opened from the keyboard; jsdom does not place it, so it is found with `hidden`. */
async function openWarranty() {
	const box = screen.getByRole('combobox', { name: 'Гарантия по умолчанию' });
	box.focus();
	await userEvent.keyboard('{Enter}');
	await screen.findAllByRole('option', { hidden: true });
	return box;
}
const optionTexts = () =>
	[...document.querySelectorAll('[role="option"]')].map((o) => o.textContent?.trim());
function option(text: string): HTMLElement {
	const found = [...document.querySelectorAll<HTMLElement>('[role="option"]')].find(
		(o) => o.textContent?.trim() === text
	);
	if (!found) throw new Error(`no option «${text}»`);
	return found;
}

describe('«Гарантия по умолчанию»', () => {
	it('offers «Не задана» and the warranties of the dictionary that are not archived', async () => {
		render(Page);
		await waitFor(() => expect(api.listDictionary).toHaveBeenCalledWith('warranty'));
		const box = screen.getByRole('combobox', { name: 'Гарантия по умолчанию' });
		expect(box).toHaveTextContent('30 дней');
		await waitFor(async () => {
			await openWarranty();
			expect(optionTexts()).toEqual(['Не задана', '14 дней', '30 дней']);
		});
		await userEvent.click(option('14 дней'));
		await waitFor(() =>
			expect(api.updateSettings).toHaveBeenCalledWith({ default_warranty: '14 дней' })
		);
	});

	it('«Не задана» saves an empty warranty', async () => {
		render(Page);
		await waitFor(() => expect(api.listDictionary).toHaveBeenCalledWith('warranty'));
		await openWarranty();
		await userEvent.click(option('Не задана'));
		await waitFor(() => expect(api.updateSettings).toHaveBeenCalledWith({ default_warranty: '' }));
	});

	it('an empty saved warranty shows «Не задана»', async () => {
		state.settings = { ...state.settings, default_warranty: '' };
		render(Page);
		await waitFor(() => expect(api.listDictionary).toHaveBeenCalled());
		expect(screen.getByRole('combobox', { name: 'Гарантия по умолчанию' })).toHaveTextContent(
			'Не задана'
		);
	});

	it('a saved value that the dictionary does not have stays, marked so', async () => {
		state.settings = { ...state.settings, default_warranty: '45 суток' };
		render(Page);
		const box = screen.getByRole('combobox', { name: 'Гарантия по умолчанию' });
		await waitFor(() => expect(box).toHaveTextContent('45 суток (нет в справочнике)'));
		await openWarranty();
		expect(optionTexts()).toEqual([
			'Не задана',
			'45 суток (нет в справочнике)',
			'14 дней',
			'30 дней'
		]);
		expect(api.updateSettings).not.toHaveBeenCalled();
	});

	it('«Что это?» says where the values are written: the dictionary, with a number and a unit', async () => {
		render(Page);
		await userEvent.click(
			screen.getByRole('button', { name: ru.help.open('Гарантия по умолчанию') })
		);
		const hint = await screen.findByRole('dialog', { name: 'Гарантия по умолчанию' });
		const where = `«Настройки → ${ru.dictionaries.kinds} → ${ru.dictionaries.kindNames.warranty}»`;
		expect(hint).toHaveTextContent(where);
		expect(hint).toHaveTextContent('«30 дней», «3 мес.»');
		expect(hint).not.toHaveTextContent('пишите его');
	});

	it('the dictionary not read: the saved value is shown as it is, without a mark', async () => {
		api.listDictionary.mockRejectedValue(new TypeError('Failed to fetch'));
		state.settings = { ...state.settings, default_warranty: '45 суток' };
		render(Page);
		await waitFor(() => expect(api.listDictionary).toHaveBeenCalled());
		expect(screen.getByRole('combobox', { name: 'Гарантия по умолчанию' })).toHaveTextContent(
			/^45 суток$/
		);
	});
});
