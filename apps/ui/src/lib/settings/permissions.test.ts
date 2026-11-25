import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import type { Permission } from '$api/Permission';
import { ru } from '#lib/i18n/ru.js';
import { PERMISSION_GROUPS } from './permissions.js';

// The names of the rights are the server's (`Permission::label_ru`), and each right is in exactly one group.
const source = readFileSync(
	resolve(process.cwd(), '../../crates/mw-domain/src/permission.rs'),
	'utf8'
);
const labelBlock = source.slice(source.indexOf('pub fn label_ru'));
const serverLabels = new Map(
	[...labelBlock.matchAll(/Self::(\w+) => "([^"]+)"/g)].map((m) => [
		m[1]!.replace(/([a-z])([A-Z])/g, '$1_$2').toLowerCase(),
		m[2]!
	])
);

describe('permissions', () => {
	it('names every right as the server does', () => {
		expect(serverLabels.size).toBe(18);
		for (const [code, label] of serverLabels) {
			expect(ru.permissions[code as Permission].label, code).toBe(label);
		}
	});

	it('puts each right in one group and says what it allows', () => {
		const all = PERMISSION_GROUPS.flatMap((g) => g.permissions);
		expect(new Set(all).size).toBe(all.length);
		expect([...all].sort()).toEqual([...serverLabels.keys()].sort());
		for (const code of all) expect(ru.permissions[code].hint.length, code).toBeGreaterThan(0);
	});

	it('keeps «Выгрузка» with the reports', () => {
		expect(PERMISSION_GROUPS.find((g) => g.id === 'reports')?.permissions).toEqual([
			'reports_view',
			'data_export'
		]);
	});

	it('names users with the right ending', () => {
		const f = ru.roles.usersCount;
		expect([0, 1, 2, 5, 11, 21, 24, 112].map(f)).toEqual([
			'нет пользователей',
			'1 пользователь',
			'2 пользователя',
			'5 пользователей',
			'11 пользователей',
			'21 пользователь',
			'24 пользователя',
			'112 пользователей'
		]);
	});
});
