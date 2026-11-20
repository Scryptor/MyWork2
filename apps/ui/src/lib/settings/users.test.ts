import { describe, expect, it } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import { signInLine, userFailure, userFieldErrors, userTitle } from './users.js';

const lockout = new ApiError(422, 'validation', 'Нельзя', {
	code: 'validation',
	message: 'Нельзя',
	fields: [{ field: 'role_id', code: 'last_admin', message: 'Это последний администратор' }]
} as never);

describe('users helpers', () => {
	it('puts the server text at its field', () => {
		expect(userFieldErrors(lockout)).toEqual({ role_id: 'Это последний администратор' });
		expect(userFailure(lockout)).toBe('Это последний администратор');
		expect(userFieldErrors(new Error('x'))).toEqual({});
	});

	it('names a user by the display name, else the login', () => {
		expect(userTitle({ display_name: 'Олег', login: 'oleg' })).toBe('Олег');
		expect(userTitle({ display_name: '', login: 'oleg' })).toBe('oleg');
	});

	it('says why a user does not sign in', () => {
		expect(signInLine({ login: '', can_sign_in: false, has_password: false })).toBe('без входа');
		expect(signInLine({ login: 'ira', can_sign_in: true, has_password: true })).toBe('ira');
		expect(signInLine({ login: 'ira', can_sign_in: true, has_password: false })).toBe(
			'ira · без пароля'
		);
	});
});
