import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';

// «Пользователи» as a page: the list with «без входа» and «без пароля», the disabled in a group of their own,
// no controls without «users_manage», and the server's lock-out text at the role.
const api = vi.hoisted(() => ({
	listUsers: vi.fn(),
	listRoles: vi.fn(),
	createUser: vi.fn(),
	updateUser: vi.fn(),
	setUserPassword: vi.fn()
}));
vi.mock('#lib/api/users.js', () => api);
const rights = vi.hoisted(() => ({ manage: true }));
vi.mock('#lib/app/context.svelte.js', () => ({ app: { can: () => rights.manage } }));

import Page from './UsersPageHarness.test-helper.svelte';

const user = (id: number, over: Record<string, unknown> = {}) => ({
	id,
	login: `u${id}`,
	display_name: `Имя ${id}`,
	role_id: 1,
	active: true,
	can_sign_in: true,
	is_master: false,
	has_password: true,
	created_at: '2026-01-01T00:00:00',
	version: 1,
	...over
});

beforeEach(() => {
	vi.clearAllMocks();
	rights.manage = true;
	api.listRoles.mockResolvedValue([
		{ id: 1, name: 'Администратор', permissions: [], version: 1 },
		{ id: 2, name: 'Мастер', permissions: [], version: 1 }
	]);
});

describe('«Пользователи»', () => {
	it('lists users, marks no sign-in and no password, groups the disabled', async () => {
		api.listUsers.mockResolvedValue([
			user(1),
			user(2, { login: '', can_sign_in: false, is_master: true, has_password: false }),
			user(3, { has_password: false }),
			user(4, { active: false })
		]);
		render(Page);
		const main = await screen.findByRole('list', { name: 'Пользователи' });
		expect(within(main).getAllByRole('listitem')).toHaveLength(3);
		expect(within(main).getByText('без входа')).toBeInTheDocument();
		expect(within(main).getByText('Мастер', { selector: 'span.badge' })).toBeInTheDocument();
		expect(within(main).getByText(/без пароля/)).toBeInTheDocument();
		const off = screen.getByRole('list', { name: 'Отключённые' });
		expect(within(off).getByText('Имя 4')).toBeInTheDocument();
	});

	it('without «users_manage» shows only the notice and asks for nothing', () => {
		rights.manage = false;
		render(Page);
		expect(screen.getByText('Пользователей настраивает администратор')).toBeInTheDocument();
		expect(screen.queryByRole('button', { name: 'Добавить пользователя' })).toBeNull();
		expect(api.listUsers).not.toHaveBeenCalled();
	});

	it('shows the server text of a lock-out at «Отключить» and keeps the user active', async () => {
		api.listUsers.mockResolvedValue([user(1)]);
		api.updateUser.mockRejectedValue(
			new ApiError(422, 'validation', 'Нельзя', {
				code: 'validation',
				message: 'Нельзя',
				fields: [{ field: 'active', code: 'last_admin', message: 'Это последний администратор' }]
			} as never)
		);
		const ui = userEvent.setup();
		render(Page);
		await ui.click(await screen.findByRole('button', { name: 'Изменить: Имя 1' }));
		const dialog = await screen.findByRole('dialog', { name: 'Пользователь' });
		await ui.click(within(dialog).getByRole('button', { name: 'Отключить учётную запись' }));
		await waitFor(() =>
			expect(within(dialog).getByText('Это последний администратор')).toBeInTheDocument()
		);
		expect(api.updateUser).toHaveBeenCalledWith(1, 1, { active: false });
		expect(
			within(dialog).getByRole('button', { name: 'Отключить учётную запись' })
		).toBeInTheDocument();
	});

	it('adds a user without sign-in: no login or password is sent', async () => {
		api.listUsers.mockResolvedValue([]);
		api.createUser.mockResolvedValue(
			user(9, { login: '', can_sign_in: false, display_name: 'Олег' })
		);
		const ui = userEvent.setup();
		render(Page);
		await ui.click(await screen.findByRole('button', { name: 'Добавить пользователя' }));
		const dialog = await screen.findByRole('dialog', { name: 'Новый пользователь' });
		expect(within(dialog).queryByLabelText(/Логин/)).toBeNull();
		await ui.type(within(dialog).getByRole('textbox', { name: /Имя/ }), 'Олег');
		await ui.click(within(dialog).getByRole('button', { name: 'Добавить' }));
		await waitFor(() =>
			expect(api.createUser).toHaveBeenCalledWith({
				display_name: 'Олег',
				login: '',
				password: '',
				role_id: 2,
				can_sign_in: false,
				is_master: false
			})
		);
	});
});
