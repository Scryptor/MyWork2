import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { beforeEach, expect, it, vi } from 'vitest';
import { resetSession, settleQueues } from '#lib/app/session-reset.js';
import { addOrderComment, orderComments } from '#lib/api/orders.js';
import OrderComments from './OrderComments.svelte';

vi.mock('#lib/api/orders.js', () => ({
	addOrderComment: vi.fn(),
	orderComments: vi.fn()
}));

const entry = {
	id: 8,
	author_name: 'Мастер',
	created_at: '2026-11-21T14:45:00',
	body: 'Проверил плату'
};

beforeEach(() => {
	resetSession();
	vi.resetAllMocks();
	vi.mocked(orderComments).mockResolvedValue({ items: [], total: 0 });
});

it('keeps the draft and its attempt key after a failed send', async () => {
	vi.mocked(addOrderComment).mockRejectedValueOnce(new Error()).mockResolvedValueOnce(entry);
	render(OrderComments, { orderId: 18, canPost: true });
	const input = screen.getByRole('textbox', { name: 'Новая запись' });
	await fireEvent.input(input, { target: { value: entry.body } });
	const send = screen.getByRole('button', { name: 'Добавить запись' });
	await fireEvent.click(send);
	expect(await screen.findByRole('alert')).toBeVisible();
	expect(input).toHaveValue(entry.body);
	vi.mocked(orderComments).mockResolvedValue({ items: [entry], total: 1 });
	await fireEvent.click(send);
	await waitFor(() => expect(input).toHaveValue(''));
	expect(vi.mocked(addOrderComment).mock.calls[1]).toEqual(
		vi.mocked(addOrderComment).mock.calls[0]
	);
	expect(await screen.findByText('Мастер')).toBeVisible();
	expect(screen.getByText('21 ноября 2026, 14:45')).toBeVisible();
});

it('shows the feed without a composer to a reader', async () => {
	vi.mocked(orderComments).mockResolvedValue({ items: [entry], total: 1 });
	render(OrderComments, { orderId: 19, canPost: false });
	expect(await screen.findByText(entry.body)).toBeVisible();
	expect(screen.queryByRole('textbox')).toBeNull();
});

it('keeps its cursor and stops at the oldest entry if another master posts between pages', async () => {
	const page = [4, 3, 2, 1].map((id) => ({ ...entry, id, body: `Запись ${id}` }));
	vi.mocked(orderComments).mockImplementation(async (_orderId, limit, beforeId) => ({
		items: page.filter((note) => beforeId === undefined || note.id < beforeId).slice(0, limit),
		total: page.length
	}));
	render(OrderComments, { orderId: 22, canPost: false });
	const older = await screen.findByRole('button', { name: 'Показать более ранние записи' });
	expect(screen.getAllByRole('listitem')).toHaveLength(3);
	page.unshift({ ...entry, id: 5, body: 'Добавлено параллельно' });
	await fireEvent.click(older);
	expect(await screen.findByText('Запись 1')).toBeVisible();
	expect(screen.getAllByRole('listitem')).toHaveLength(4);
	expect(screen.queryByRole('button')).toBeNull();
	expect(orderComments).toHaveBeenLastCalledWith(22, 4, 2, expect.any(AbortSignal));
});

it('keeps an unsent draft with its order when switching views', async () => {
	const first = render(OrderComments, { orderId: 24, canPost: true });
	const input = () => screen.getByRole('textbox', { name: 'Новая запись' });
	await fireEvent.input(input(), { target: { value: 'Проверить после обеда' } });
	first.unmount();
	const other = render(OrderComments, { orderId: 25, canPost: true });
	expect(input()).toHaveValue('');
	other.unmount();
	render(OrderComments, { orderId: 24, canPost: true });
	expect(input()).toHaveValue('Проверить после обеда');
});

it('does not duplicate a send when the same order is reopened before the reply', async () => {
	const pending = Promise.withResolvers<typeof entry>();
	vi.mocked(addOrderComment).mockReturnValue(pending.promise);
	const panel = render(OrderComments, { orderId: 26, canPost: true });
	await fireEvent.input(screen.getByRole('textbox'), { target: { value: entry.body } });
	await fireEvent.click(screen.getByRole('button', { name: 'Добавить запись' }));
	panel.unmount();
	render(OrderComments, { orderId: 26, canPost: true });
	expect(screen.getByRole('textbox')).toBeDisabled();
	vi.mocked(orderComments).mockResolvedValue({ items: [entry], total: 1 });
	pending.resolve(entry);
	expect(await screen.findByText(entry.body)).toBeVisible();
	expect(screen.getByRole('textbox')).toHaveValue('');
	expect(addOrderComment).toHaveBeenCalledTimes(1);
});

it('drops the previous session draft and ignores its delayed response', async () => {
	const pending = Promise.withResolvers<typeof entry>();
	vi.mocked(addOrderComment).mockReturnValue(pending.promise);
	const panel = render(OrderComments, { orderId: 28, canPost: true });
	await fireEvent.input(screen.getByRole('textbox'), { target: { value: 'Старый черновик' } });
	expect(await settleQueues()).toBe(false);
	await fireEvent.click(screen.getByRole('button', { name: 'Добавить запись' }));
	panel.unmount();
	resetSession();
	render(OrderComments, { orderId: 28, canPost: true });
	const field = screen.getByRole('textbox');
	expect(field).toHaveValue('');
	await fireEvent.input(field, { target: { value: 'Новый сеанс' } });
	pending.resolve(entry);
	await pending.promise;
	expect(field).toHaveValue('Новый сеанс');
});
