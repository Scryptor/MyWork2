import { describe, expect, it } from 'vitest';
import { resetSession } from './session-reset.js';
import { queueLoginNotice, takeLoginNotices } from './login-notice.js';

describe('login notices', () => {
	it('a notice queued before the login page resets the session is still there after it', () => {
		queueLoginNotice({ message: 'Сеанс истёк — войдите снова' });
		resetSession();
		expect(takeLoginNotices()).toEqual([{ message: 'Сеанс истёк — войдите снова' }]);
	});

	it('is shown once', () => {
		queueLoginNotice({ message: 'a', tone: 'success' });
		expect(takeLoginNotices()).toHaveLength(1);
		expect(takeLoginNotices()).toEqual([]);
	});
});
