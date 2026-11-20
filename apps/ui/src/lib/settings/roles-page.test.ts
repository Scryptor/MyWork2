import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';

// «Роли» as a page: roles with the number of their users, rights as checkboxes in groups for any role, the
// server's refusals (last «Пользователи и права», users in the role) shown and the checkbox put back.
const api = vi.hoisted(() => ({
	listUsers: vi.fn(),
	listRoles: vi.fn(),
	createRole: vi.fn(),
	updateRole: vi.fn(),
	deleteRole: vi.fn()
}));
vi.mock('#lib/api/users.js', () => api);
const rights = vi.hoisted(() => ({ manage: true }));
vi.mock('#lib/app/context.svelte.js', () => ({ app: { can: () => rights.manage } }));

import Page from './RolesPageHarness.test-helper.svelte';

const role = (id: number, name: string, permissions: string[] = [], version = 1) => ({
	id,
	name,
	permissions,
	version
});
const user = (id: number, role_id: number) => ({ id, role_id });

beforeEach(() => {
	vi.clearAllMocks();
	rights.manage = true;
	api.listRoles.mockResolvedValue([
		role(1, 'Администратор', ['users_manage', 'settings_manage']),
		role(2, 'Мастер', ['orders_edit']),
		role(3, 'Пустая')
	]);
	api.listUsers.mockResolvedValue([user(1, 1), user(2, 2), user(3, 2)]);
});

const open = async (ui: ReturnType<typeof userEvent.setup>, name: string) => {
	await ui.click(await screen.findByRole('button', { name: `Изменить: ${name}` }));
	return screen.findByRole('dialog', { name: 'Роль' });
};

describe('«Роли»', () => {
	it('lists roles with the number of users', async () => {
		render(Page);
		const list = await screen.findByRole('list', { name: 'Роли' });
		const rows = within(list).getAllByRole('listitem');
		expect(rows).toHaveLength(3);
		expect(rows[0]).toHaveTextContent('1 пользователь');
		expect(rows[1]).toHaveTextContent('2 пользователя');
		expect(rows[2]).toHaveTextContent('нет пользователей');
	});

	it('without «users_manage» shows only the notice and asks for nothing', () => {
		rights.manage = false;
		render(Page);
		expect(screen.getByText('Роли настраивает администратор')).toBeInTheDocument();
		expect(screen.queryByRole('button', { name: 'Добавить роль' })).toBeNull();
		expect(api.listRoles).not.toHaveBeenCalled();
	});

	it('shows the rights of a built-in role in groups and saves a ticked one at once', async () => {
		api.updateRole.mockResolvedValue(role(2, 'Мастер', ['orders_create', 'orders_edit'], 2));
		const ui = userEvent.setup();
		render(Page);
		const dialog = await open(ui, 'Мастер');
		for (const g of ['Заказы', 'Клиенты', 'Склад', 'Отчёты', 'Настройки и пользователи']) {
			expect(within(dialog).getByRole('group', { name: g })).toBeInTheDocument();
		}
		expect(within(dialog).getAllByRole('checkbox')).toHaveLength(17);
		expect(within(dialog).getByRole('checkbox', { name: 'Изменение заказов' })).toBeChecked();
		expect(within(dialog).getByRole('checkbox', { name: 'Отчёты' })).not.toBeChecked();
		await ui.click(within(dialog).getByRole('checkbox', { name: 'Добавление заказов' }));
		await waitFor(() =>
			expect(api.updateRole).toHaveBeenCalledWith(2, 1, {
				permissions: ['orders_create', 'orders_edit']
			})
		);
		await waitFor(() =>
			expect(within(dialog).getByRole('checkbox', { name: 'Добавление заказов' })).toBeChecked()
		);
	});

	it('has a hint per right', async () => {
		const ui = userEvent.setup();
		render(Page);
		const dialog = await open(ui, 'Мастер');
		await ui.click(within(dialog).getByRole('button', { name: 'Что такое «Просмотр склада»' }));
		expect(
			await screen.findByText(/Пока ничего не открывает: склада в программе ещё нет/)
		).toBeInTheDocument();
	});

	it('shows the server text when the last «Пользователи и права» is taken off and puts the box back', async () => {
		api.updateRole.mockRejectedValue(
			new ApiError(
				409,
				'conflict',
				'Нельзя снять право «Пользователи и права» с последней роли, где оно есть',
				{
					code: 'in_use',
					message: 'Нельзя снять право «Пользователи и права» с последней роли, где оно есть'
				} as never
			)
		);
		const ui = userEvent.setup();
		render(Page);
		const dialog = await open(ui, 'Администратор');
		const box = within(dialog).getByRole('checkbox', { name: 'Пользователи и права' });
		expect(box).toBeChecked();
		await ui.click(box);
		await waitFor(() =>
			expect(within(dialog).getByRole('alert')).toHaveTextContent(
				'Нельзя снять право «Пользователи и права» с последней роли'
			)
		);
		expect(within(dialog).getByRole('checkbox', { name: 'Пользователи и права' })).toBeChecked();
	});

	it('shows a 422 lock-out of a sign-in administrator and puts the box back', async () => {
		api.updateRole.mockRejectedValue(
			new ApiError(422, 'validation', 'Нельзя', {
				code: 'validation',
				message: 'Нельзя',
				fields: [
					{
						field: 'permissions',
						code: 'last_admin',
						message: 'Нельзя оставить программу без администратора'
					}
				]
			} as never)
		);
		const ui = userEvent.setup();
		render(Page);
		const dialog = await open(ui, 'Администратор');
		await ui.click(within(dialog).getByRole('checkbox', { name: 'Пользователи и права' }));
		await waitFor(() =>
			expect(within(dialog).getByRole('alert')).toHaveTextContent(
				'Нельзя оставить программу без администратора'
			)
		);
		expect(within(dialog).getByRole('checkbox', { name: 'Пользователи и права' })).toBeChecked();
	});

	it('does not delete a role with users and explains why; deletes an empty one', async () => {
		api.deleteRole.mockResolvedValue(undefined);
		const ui = userEvent.setup();
		render(Page);
		let dialog = await open(ui, 'Мастер');
		expect(within(dialog).getByRole('button', { name: 'Удалить роль' })).toBeDisabled();
		expect(within(dialog).getByText(/пока в ней есть пользователи: 2/)).toBeInTheDocument();
		await ui.keyboard('{Escape}');
		await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Роль' })).toBeNull());
		dialog = await open(ui, 'Пустая');
		await ui.click(within(dialog).getByRole('button', { name: 'Удалить роль' }));
		await ui.click(
			await within(await screen.findByRole('alertdialog')).findByRole('button', {
				name: 'Удалить'
			})
		);
		await waitFor(() => expect(api.deleteRole).toHaveBeenCalledWith(3, 1));
		await waitFor(() =>
			expect(screen.queryByRole('button', { name: 'Изменить: Пустая' })).toBeNull()
		);
	});

	it('adds a role with a name and ticked rights', async () => {
		api.createRole.mockResolvedValue(role(9, 'Курьер', ['orders_edit', 'clients_view']));
		const ui = userEvent.setup();
		render(Page);
		await ui.click(await screen.findByRole('button', { name: 'Добавить роль' }));
		const dialog = await screen.findByRole('dialog', { name: 'Новая роль' });
		await ui.click(within(dialog).getByRole('button', { name: 'Добавить' }));
		expect(within(dialog).getByText('Введите название роли')).toBeInTheDocument();
		expect(api.createRole).not.toHaveBeenCalled();
		await ui.type(within(dialog).getByRole('textbox', { name: 'Название' }), 'Курьер');
		await ui.click(within(dialog).getByRole('checkbox', { name: 'Изменение заказов' }));
		await ui.click(within(dialog).getByRole('checkbox', { name: 'Просмотр клиентов' }));
		await ui.click(within(dialog).getByRole('button', { name: 'Добавить' }));
		await waitFor(() =>
			expect(api.createRole).toHaveBeenCalledWith({
				name: 'Курьер',
				permissions: ['orders_edit', 'clients_view']
			})
		);
		expect(await screen.findByRole('button', { name: 'Изменить: Курьер' })).toBeInTheDocument();
	});
	it("after a real conflict retries on the reloaded role: another administrator's right is kept", async () => {
		api.updateRole
			.mockRejectedValueOnce(new ApiError(409, 'conflict', 'Версия устарела', null))
			.mockResolvedValueOnce(
				role(2, 'Мастер', ['orders_create', 'orders_edit', 'clients_view'], 3)
			);
		// The other administrator added «clients_view» meanwhile: the reload shows it.
		api.listRoles
			.mockResolvedValueOnce([
				role(1, 'Администратор'),
				role(2, 'Мастер', ['orders_edit']),
				role(3, 'Пустая')
			])
			.mockResolvedValue([
				role(1, 'Администратор'),
				role(2, 'Мастер', ['orders_edit', 'clients_view'], 2),
				role(3, 'Пустая')
			]);
		const ui = userEvent.setup();
		render(Page);
		const dialog = await open(ui, 'Мастер');
		await ui.click(within(dialog).getByRole('checkbox', { name: 'Добавление заказов' }));
		await waitFor(() => expect(api.updateRole).toHaveBeenCalledTimes(2));
		expect(api.updateRole.mock.calls[0]).toEqual([
			2,
			1,
			{ permissions: ['orders_create', 'orders_edit'] }
		]);
		expect(api.updateRole.mock.calls[1]).toEqual([
			2,
			2,
			{ permissions: ['orders_create', 'orders_edit', 'clients_view'] }
		]);
	});

	it('keeps the focus on the ticked box, also after a refusal', async () => {
		api.updateRole.mockResolvedValueOnce(role(2, 'Мастер', ['orders_create', 'orders_edit'], 2));
		const ui = userEvent.setup();
		render(Page);
		const dialog = await open(ui, 'Мастер');
		await ui.click(within(dialog).getByRole('checkbox', { name: 'Добавление заказов' }));
		await waitFor(() => expect(api.updateRole).toHaveBeenCalledTimes(1));
		expect(within(dialog).getByRole('checkbox', { name: 'Добавление заказов' })).toHaveFocus();
		api.updateRole.mockRejectedValueOnce(
			new ApiError(
				409,
				'conflict',
				'Нельзя снять право «Пользователи и права» с последней роли, где оно есть',
				null
			)
		);
		api.updateRole.mockRejectedValueOnce(new ApiError(409, 'conflict', 'Нельзя', null));
		await ui.click(within(dialog).getByRole('checkbox', { name: 'Изменение заказов' }));
		await waitFor(() => expect(within(dialog).getByRole('alert')).toBeInTheDocument());
		await waitFor(() =>
			expect(within(dialog).getByRole('checkbox', { name: 'Изменение заказов' })).toHaveFocus()
		);
	});
});
