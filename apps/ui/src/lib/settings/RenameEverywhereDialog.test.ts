import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import RenameEverywhereDialog from './RenameEverywhereDialog.svelte';
import MergeDialog from './MergeDialog.svelte';

/** A promise the test settles by hand: the request that "takes a moment". */
function pending() {
	let done!: (v: string | null) => void;
	const promise = new Promise<string | null>((r) => (done = r));
	return { promise, done };
}

// Esc and a click outside while the request runs are checked in e2e (settings-dictionaries.spec.ts):
// jsdom does not tell a dismissible dialog from one that is not, so here only the buttons are.
describe('RenameEverywhereDialog while the request runs', () => {
	it('is busy: Cancel and ✕ are disabled, and it closes when the request ends', async () => {
		const request = pending();
		render(RenameEverywhereDialog, {
			open: true,
			from: 'Alpha',
			to: 'Gamma',
			where: 'в 3 заказах',
			onconfirm: () => request.promise
		});
		const dialog = screen.getByRole('dialog', { name: 'Переименовать „Alpha“ в „Gamma“' });
		await userEvent.click(screen.getByRole('button', { name: 'Переименовать' }));

		// The busy button hides its text behind a spinner: found by its state.
		await waitFor(() => expect(document.querySelector('button[aria-busy="true"]')).not.toBeNull());
		expect(screen.getByRole('button', { name: 'Отмена' })).toBeDisabled();
		expect(screen.getByRole('button', { name: 'Закрыть' })).toBeDisabled();
		expect(dialog).toBeInTheDocument();

		request.done(null);
		await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
	});

	it('stays open with the error when the request fails', async () => {
		render(RenameEverywhereDialog, {
			open: true,
			from: 'A',
			to: 'B',
			where: 'в 1 заказе',
			onconfirm: async () => 'Такое значение уже есть — объедините их'
		});
		await userEvent.click(screen.getByRole('button', { name: 'Переименовать' }));
		expect(await screen.findByText('Такое значение уже есть — объедините их')).toBeInTheDocument();
		expect(screen.getByRole('dialog')).toBeInTheDocument();
	});
});

describe('the dialog without a request', () => {
	it('closes with Esc (the control of the busy tests)', async () => {
		render(RenameEverywhereDialog, {
			open: true,
			from: 'A',
			to: 'B',
			where: 'в 1 заказе',
			onconfirm: async () => null
		});
		await userEvent.keyboard('{Escape}');
		await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
	});
});

describe('MergeDialog while the request runs', () => {
	it('Cancel and ✕ are disabled until the request ends', async () => {
		const request = pending();
		render(MergeDialog, {
			open: true,
			rows: [
				{ id: 1, label: 'Alpha', count: 1 },
				{ id: 2, label: 'Beta', count: 0 }
			],
			countLabel: (n: number) => `в ${n} заказах`,
			where: 'в заказах',
			onconfirm: () => request.promise
		});
		await userEvent.click(screen.getByRole('button', { name: 'Объединить' }));
		await waitFor(() => expect(screen.getByRole('button', { name: 'Отмена' })).toBeDisabled());
		expect(screen.getByRole('button', { name: 'Закрыть' })).toBeDisabled();
		expect(screen.getByRole('dialog')).toBeInTheDocument();
		request.done(null);
		await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
	});
});

describe('a warranty value', () => {
	// The term of an order comes from the text of its warranty: the orders keep theirs.
	const NOTE =
		'Срок гарантии в заказах выводится из текста. Заменить в заказах можно, только если срок не меняется: например, „12 мес.“ → „1 год“.';

	it('rename: «И в заказах» is off by default and the note says why', async () => {
		let asked: boolean | null = null;
		render(RenameEverywhereDialog, {
			open: true,
			from: '30 дней',
			to: '60 дней',
			where: 'в 3 заказах',
			warranty: true,
			onconfirm: async (inOrders: boolean) => ((asked = inOrders), null)
		});
		expect(screen.getByRole('checkbox', { name: 'И в 3 заказах' })).not.toBeChecked();
		expect(screen.getByText(NOTE)).toBeInTheDocument();
		await userEvent.click(screen.getByRole('button', { name: 'Переименовать' }));
		await waitFor(() => expect(asked).toBe(false));
	});

	it('any other value: on by default, no note', () => {
		render(RenameEverywhereDialog, {
			open: true,
			from: 'A',
			to: 'B',
			where: 'в 3 заказах',
			onconfirm: async () => null
		});
		expect(screen.getByRole('checkbox', { name: 'И в 3 заказах' })).toBeChecked();
		expect(screen.queryByText(NOTE)).not.toBeInTheDocument();
	});

	it('merge: «Заменить и в заказах» is off by default and the note says why', () => {
		render(MergeDialog, {
			open: true,
			rows: [
				{ id: 1, label: '12 мес.', count: 1 },
				{ id: 2, label: '1 год', count: 4 }
			],
			countLabel: (n: number) => `в ${n} заказах`,
			where: 'в заказах',
			warranty: true,
			onconfirm: async () => null
		});
		expect(screen.getByRole('checkbox', { name: 'Заменить и в заказах' })).not.toBeChecked();
		expect(screen.getByText(NOTE)).toBeInTheDocument();
	});
});
