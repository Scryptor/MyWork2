import { ApiError, errorMessage } from '#lib/api/client.js';
import { ru } from '#lib/i18n/ru.js';

export interface PasswordForm {
	current: string;
	next: string;
	repeat: string;
}

/** Errors of «Сменить пароль»: at a field, or of the whole form. */
export interface PasswordErrors {
	current?: string;
	next?: string;
	repeat?: string;
	form?: string;
}

/** What is wrong before asking the server: empty fields, a repeat that differs. */
export function checkPasswordForm(f: PasswordForm): PasswordErrors {
	const found: PasswordErrors = {};
	if (!f.current) found.current = ru.password.required;
	if (!f.next) found.next = ru.password.required;
	if (!f.repeat) found.repeat = ru.password.required;
	else if (f.next && f.repeat !== f.next) found.repeat = ru.password.mismatch;
	return found;
}

/** The server's answer at the fields it is about; anything else is a message of the form. */
export function passwordErrorsFromServer(e: unknown): PasswordErrors {
	if (!(e instanceof ApiError)) return { form: errorMessage(e) };
	if (e.code === 'too_many_attempts') {
		return { current: ru.auth.tooManyAttempts(e.body?.retry_after_secs ?? 60) };
	}
	if (e.code !== 'validation') return { form: errorMessage(e) };
	const found: PasswordErrors = {};
	for (const f of e.body?.fields ?? []) {
		if (f.field === 'current_password') {
			found.current = f.code === 'invalid' ? ru.password.wrongCurrent : f.message;
		} else if (f.field === 'new_password') {
			found.next = f.code === 'too_short' ? ru.password.tooShort : f.message;
		}
	}
	return Object.keys(found).length > 0 ? found : { form: errorMessage(e) };
}
