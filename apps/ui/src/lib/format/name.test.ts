import { describe, expect, it } from 'vitest';
import { displayName, initials, shortName } from './name.js';

describe('name', () => {
	it('displayName', () => {
		expect(displayName('ИВАНОВ ИВАН')).toBe('Иванов Иван');
		expect(displayName('  ПЕТРОВ   ВИКТОР ')).toBe('Петров Виктор');
		expect(displayName('САЛТЫКОВ-ЩЕДРИН МИХАИЛ')).toBe('Салтыков-Щедрин Михаил');
		expect(displayName('Иванов иван')).toBe('Иванов иван');
		expect(displayName('ООО РОМАШКА')).toBe('ООО Ромашка');
		expect(displayName('IVANOV IVAN')).toBe('Ivanov Ivan');
		expect(displayName('')).toBe('');
	});
	it('shortName', () => {
		expect(shortName('ИВАНОВ ИВАН ИВАНОВИЧ')).toBe('Иванов И. И.');
		expect(shortName('ИВАНОВ')).toBe('Иванов');
		expect(shortName('ООО РОМАШКА')).toBe('ООО Ромашка');
	});
	it('initials', () => {
		expect(initials('ИВАНОВ ИВАН')).toBe('ИИ');
		expect(initials('')).toBe('?');
	});
});
