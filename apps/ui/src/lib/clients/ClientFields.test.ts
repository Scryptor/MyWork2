import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { client } from '#lib/app/fixtures.test-helper.js';
import { createAutosave } from '#lib/orders/autosave.js';
import ClientFields from './ClientFieldsHarness.test-helper.svelte';
import type { ClientConflict, ClientField, ClientValues } from './editor.svelte.js';

const PHONES = [
	{ phone: '79211234567', note: '' },
	{ phone: '79001112233', note: 'раб' }
];

/**
 * A real autosave whose saves only record what went: the fields read their values from it as in the
 * panel. `asked` lists every phone list the fields gave it, `saved` every patch sent.
 */
function recordingAutosave() {
	const saved: Partial<ClientValues>[] = [];
	const autosave = createAutosave<ClientValues>({
		delayMs: 600,
		save: async (patch) => void saved.push(patch)
	});
	const edit = vi.spyOn(autosave, 'edit');
	const commit = vi.spyOn(autosave, 'commit');
	const asked = () =>
		[...edit.mock.calls, ...commit.mock.calls]
			.filter(([field]) => field === 'phones')
			.map(([, value]) => value as typeof PHONES);
	return { autosave, saved, asked };
}

describe('ClientFields', () => {
	it('a number erased with Backspace is never sent in pieces; the old one stays', async () => {
		const { autosave, saved, asked } = recordingAutosave();
		render(ClientFields, { client: client(5, { phones: PHONES }), autosave, sources: [] });
		const number = screen.getByRole('textbox', { name: 'Номер 2' });
		await userEvent.click(number);
		await userEvent.keyboard('{End}');
		for (let i = 0; i < 20; i++) await userEvent.keyboard('{Backspace}');
		expect(number).toHaveValue('');
		// Every list asked for has whole numbers only: from 5 digits on, then the old number again.
		for (const list of asked())
			for (const p of list) expect(p.phone.length).toBeGreaterThanOrEqual(5);
		expect(asked().at(-1)).toEqual(PHONES);
		await userEvent.tab();
		await autosave.flush();
		for (const patch of saved)
			for (const p of patch.phones ?? []) expect(p.phone.length).toBeGreaterThanOrEqual(5);
		expect(number).toHaveValue('+7 900 111-22-33');

		// A new whole number goes.
		await userEvent.click(number);
		await userEvent.clear(number);
		await userEvent.type(number, '+7 911 000-11-22');
		expect(asked().at(-1)).toEqual([PHONES[0], { phone: '79110001122', note: 'раб' }]);
		autosave.dispose();
	});

	it('disputed phones are read-only until «Оставить моё» or «Взять новое» decides', async () => {
		const { autosave, asked } = recordingAutosave();
		const onkeep = vi.fn();
		const ontake = vi.fn();
		const conflicts = new Map<ClientField, ClientConflict>([
			['phones', { mine: PHONES, theirs: [PHONES[0]!] }]
		]);
		render(ClientFields, {
			client: client(5, { phones: PHONES }),
			autosave,
			sources: [],
			conflicts,
			onkeep,
			ontake
		});
		expect(screen.getByRole('textbox', { name: 'Телефон' })).toBeDisabled();
		expect(screen.getByRole('textbox', { name: 'Пометка 2' })).toBeDisabled();
		expect(screen.queryByRole('button', { name: '＋ номер' })).toBeNull();
		expect(screen.queryByRole('button', { name: 'Сделать основным' })).toBeNull();
		expect(screen.queryByRole('button', { name: /^Убрать номер/ })).toBeNull();
		// The other fields stay editable.
		expect(screen.getByRole('textbox', { name: 'ФИО' })).toBeEnabled();

		const dispute = screen.getByRole('group', { name: /^Телефоны: изменили/ });
		await userEvent.click(within(dispute).getByRole('button', { name: 'Оставить моё' }));
		expect(onkeep).toHaveBeenCalledWith('phones');
		await userEvent.click(within(dispute).getByRole('button', { name: 'Взять новое' }));
		expect(ontake).toHaveBeenCalledWith('phones');
		expect(asked()).toEqual([]);
		autosave.dispose();
	});
});
