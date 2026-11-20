import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

// «Настройки → Заказы»: «Гарантия по умолчанию» is a list of the dictionary of warranties, with «Не задана» first;
// a saved value that the dictionary does not have stays shown and chosen.
const api = vi.hoisted(() => ({ listDictionary: vi.fn(), updateSettings: vi.fn() }));
vi.mock('#lib/api/catalogs.js', () => api);
const state = vi.hoisted(() => ({
	settings: {
		default_service_point_id: null,
		default_master: '',
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
	state.settings = { ...state.settings, default_warranty: '30 дней' };
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
