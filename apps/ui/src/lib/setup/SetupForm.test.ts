import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';

const api = vi.hoisted(() => ({ runSetup: vi.fn() }));
vi.mock('#lib/api/setup.js', () => api);

import SetupForm from './SetupFormHarness.test-helper.svelte';

// A block body: a function returned from beforeEach is called as its cleanup.
beforeEach(() => {
	api.runSetup.mockReset();
});

// The two required fields carry «*» in their names.
const field = (name: string) => screen.getByRole('textbox', { name: new RegExp(`^${name}`) });
const submit = () => userEvent.click(screen.getByRole('button', { name: 'Начать работу' }));

describe('SetupForm («Начать с пустой базы»)', () => {
	it('the name and the point are required: nothing is sent without them', async () => {
		render(SetupForm, { ondone: vi.fn(), onalready: vi.fn() });
		await submit();
		expect(field('Название сервиса')).toHaveAttribute('aria-invalid', 'true');
		expect(field('Первая точка')).toHaveAttribute('aria-invalid', 'true');
		expect(screen.getAllByText('Обязательное поле')).toHaveLength(2);
		expect(api.runSetup).not.toHaveBeenCalled();
	});

	it('sends the name, the point and the masters, typed ones too, then says it is done', async () => {
		api.runSetup.mockResolvedValue(undefined);
		const ondone = vi.fn();
		render(SetupForm, { ondone, onalready: vi.fn() });
		await userEvent.type(field('Название сервиса'), '  Сервис-Плюс ');
		await userEvent.type(field('Первая точка'), 'Мира, 5');
		await userEvent.type(field('Мастера'), 'Петров{Enter}Сидоров');
		await submit();
		expect(api.runSetup).toHaveBeenCalledWith({
			firm_name: 'Сервис-Плюс',
			point_name: 'Мира, 5',
			masters: ['Петров', 'Сидоров']
		});
		expect(ondone).toHaveBeenCalledOnce();
	});

	it('a field the server refuses gets its words under it', async () => {
		api.runSetup.mockRejectedValue(
			new ApiError(422, 'validation', 'Ошибка в полях', {
				code: 'validation',
				message: 'Ошибка в полях',
				fields: [{ field: 'point_name', code: 'required', message: 'Обязательное поле' }],
				current_version: null,
				permission: null,
				retry_after_secs: null,
				diagnostics: []
			})
		);
		const ondone = vi.fn();
		render(SetupForm, { ondone, onalready: vi.fn() });
		await userEvent.type(field('Название сервиса'), 'Сервис');
		await userEvent.type(field('Первая точка'), 'Мира');
		await submit();
		expect(field('Первая точка')).toHaveAttribute('aria-invalid', 'true');
		expect(ondone).not.toHaveBeenCalled();
	});

	it('set up already (another window did it): it goes on to the program', async () => {
		api.runSetup.mockRejectedValue(new ApiError(409, 'conflict', 'Программа уже настроена', null));
		const onalready = vi.fn();
		render(SetupForm, { ondone: vi.fn(), onalready });
		await userEvent.type(field('Название сервиса'), 'Сервис');
		await userEvent.type(field('Первая точка'), 'Мира');
		await submit();
		expect(onalready).toHaveBeenCalledWith('Программа уже настроена');
	});

	it('another failure is said under the form, and it can be sent again', async () => {
		api.runSetup.mockRejectedValueOnce(new ApiError(0, 'network', 'Нет связи с сервером', null));
		api.runSetup.mockResolvedValueOnce(undefined);
		const ondone = vi.fn();
		render(SetupForm, { ondone, onalready: vi.fn() });
		await userEvent.type(field('Название сервиса'), 'Сервис');
		await userEvent.type(field('Первая точка'), 'Мира');
		await submit();
		expect(screen.getByRole('alert')).toHaveTextContent('Нет связи с сервером');
		await submit();
		expect(ondone).toHaveBeenCalledOnce();
	});
});
