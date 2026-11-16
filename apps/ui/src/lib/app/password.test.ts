import type { ErrorBody } from '$api/ErrorBody';
import { describe, expect, it } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import { checkPasswordForm, passwordErrorsFromServer } from './password.js';

function apiError(status: number, body: Partial<ErrorBody> & Pick<ErrorBody, 'code'>): ApiError {
	const full: ErrorBody = {
		message: 'Проверьте введённые данные',
		fields: [],
		current_version: null,
		permission: null,
		retry_after_secs: null,
		...body
	};
	return new ApiError(status, full.code, full.message, full);
}

describe('checkPasswordForm', () => {
	it('every field is required', () => {
		expect(checkPasswordForm({ current: '', next: '', repeat: '' })).toEqual({
			current: 'Заполните поле',
			next: 'Заполните поле',
			repeat: 'Заполните поле'
		});
	});

	it('the repeat must match the new password', () => {
		expect(checkPasswordForm({ current: 'a', next: 'новый', repeat: 'новыи' })).toEqual({
			repeat: 'Пароли не совпадают'
		});
		expect(checkPasswordForm({ current: 'a', next: 'новый', repeat: 'новый' })).toEqual({});
	});
});

describe('passwordErrorsFromServer', () => {
	it('puts the server field errors at their fields', () => {
		const wrong = apiError(422, {
			code: 'validation',
			fields: [{ field: 'current_password', code: 'invalid', message: 'Недопустимое значение' }]
		});
		expect(passwordErrorsFromServer(wrong)).toEqual({ current: 'Неверный текущий пароль' });
		const short = apiError(422, {
			code: 'validation',
			fields: [{ field: 'new_password', code: 'too_short', message: 'Слишком короткое значение' }]
		});
		expect(passwordErrorsFromServer(short)).toEqual({ next: 'Не короче 4 символов' });
		const other = apiError(422, {
			code: 'validation',
			fields: [{ field: 'new_password', code: 'weird', message: 'Недопустимое значение' }]
		});
		expect(passwordErrorsFromServer(other)).toEqual({ next: 'Недопустимое значение' });
	});

	it('too many attempts says when to try again, under the current password', () => {
		const limited = apiError(429, {
			code: 'too_many_attempts',
			message: 'Слишком много попыток',
			retry_after_secs: 840
		});
		expect(passwordErrorsFromServer(limited)).toEqual({
			current: 'Слишком много попыток. Повторите через 840 с'
		});
	});

	it('anything else is a message of the form', () => {
		expect(
			passwordErrorsFromServer(new ApiError(0, 'network', 'Нет связи с сервером', null))
		).toEqual({ form: 'Нет связи с сервером' });
		expect(passwordErrorsFromServer(apiError(422, { code: 'validation', fields: [] }))).toEqual({
			form: 'Проверьте введённые данные'
		});
		expect(passwordErrorsFromServer(new TypeError('x'))).toEqual({
			form: 'Не удалось выполнить запрос'
		});
	});
});
