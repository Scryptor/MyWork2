// Разбор SPDX-выражений проверки лицензий: `npm test` (его же запускает `npm run licenses`).
import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { isAllowed } from './check-licenses.mjs';

describe('isAllowed', () => {
	it('пропускает разрешённые лицензии и двойные лицензии', () => {
		assert.equal(isAllowed('MIT'), true);
		assert.equal(isAllowed('MIT OR Apache-2.0'), true);
		assert.equal(isAllowed('MIT/Apache-2.0'), true);
		assert.equal(isAllowed('Apache-2.0 WITH LLVM-exception'), true);
		assert.equal(isAllowed('(MIT OR Apache-2.0) AND Unicode-3.0'), true);
	});

	it('AND связывает сильнее OR, скобки соблюдаются', () => {
		assert.equal(isAllowed('GPL-3.0 AND (MIT OR Apache-2.0)'), false);
		assert.equal(isAllowed('(MIT OR GPL-3.0) AND Apache-2.0'), true);
		assert.equal(isAllowed('MIT OR GPL-3.0 AND Apache-2.0'), true);
		assert.equal(isAllowed('MIT AND GPL-3.0 OR MPL-2.0'), false);
	});

	it('исключение WITH — часть лицензии', () => {
		assert.equal(isAllowed('GPL-2.0 WITH Classpath-exception-2.0'), false);
		assert.equal(isAllowed('MIT WITH LLVM-exception'), false);
	});

	it('отказывает копилефту, пустому и неразборчивому выражению', () => {
		assert.equal(isAllowed('MPL-2.0'), false);
		assert.equal(isAllowed(''), false);
		assert.equal(isAllowed(undefined), false);
		assert.equal(isAllowed('(MIT OR Apache-2.0'), false);
		assert.equal(isAllowed('MIT OR'), false);
		assert.equal(isAllowed('MIT Apache-2.0'), false);
		assert.equal(isAllowed('MIT WITH'), false);
	});
});
