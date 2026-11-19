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

describe('RenameEverywhereDialog while the request runs', () => {
	it('is busy, ignores Esc and the buttons, and closes when the request ends', async () => {
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
		await userEvent.keyboard('{Escape}');
		expect(dialog).toBeInTheDocument();
		expect(screen.getByRole('dialog')).toBeInTheDocument();

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
	it('ignores Esc until the request ends', async () => {
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
		await userEvent.keyboard('{Escape}');
		expect(screen.getByRole('dialog')).toBeInTheDocument();
		request.done(null);
		await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
	});
});
