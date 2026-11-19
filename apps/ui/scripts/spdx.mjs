// SPDX-выражение лицензии для проверок `npm run licenses` (интерфейс и программа для компьютера).
// Грамматика: `AND` связывает сильнее `OR`, скобки соблюдаются, `WITH <исключение>` — часть лицензии
// (`Apache-2.0 WITH LLVM-exception` проверяется как одно имя). `/` старых манифестов — то же, что `OR`.
// Пустое или неразборчивое выражение не проходит.

/**
 * @param {unknown} expression SPDX-выражение
 * @param {(id: string) => boolean} allowed разрешена ли одна лицензия (с исключением, если оно есть)
 * @returns {boolean}
 */
export function spdxAllowed(expression, allowed) {
	const tokens = String(expression ?? '')
		.replace(/\//g, ' OR ')
		.replace(/[()]/g, ' $& ')
		.split(/\s+/)
		.filter(Boolean);
	let pos = 0;
	/** @param {string | undefined} token @param {string} word */
	const is = (token, word) => token !== undefined && token.toUpperCase() === word;
	/**
	 * @param {string | undefined} token
	 * @returns {token is string}
	 */
	const isName = (token) =>
		token !== undefined &&
		token !== '(' &&
		token !== ')' &&
		!['AND', 'OR', 'WITH'].some((w) => is(token, w));
	/** @returns {never} */
	const fail = () => {
		throw new SyntaxError(String(expression));
	};

	/** @returns {boolean} */
	function orExpr() {
		let value = andExpr();
		while (is(tokens[pos], 'OR')) {
			pos += 1;
			const right = andExpr();
			value = value || right;
		}
		return value;
	}

	/** @returns {boolean} */
	function andExpr() {
		let value = primary();
		while (is(tokens[pos], 'AND')) {
			pos += 1;
			const right = primary();
			value = value && right;
		}
		return value;
	}

	/** @returns {boolean} */
	function primary() {
		if (tokens[pos] === '(') {
			pos += 1;
			const value = orExpr();
			if (tokens[pos] !== ')') fail();
			pos += 1;
			return value;
		}
		const id = tokens[pos];
		if (!isName(id)) return fail();
		pos += 1;
		if (!is(tokens[pos], 'WITH')) return allowed(id);
		const exception = tokens[pos + 1];
		if (!isName(exception)) return fail();
		pos += 2;
		return allowed(`${id} WITH ${exception}`);
	}

	if (tokens.length === 0) return false;
	try {
		const value = orExpr();
		return pos === tokens.length && value;
	} catch (e) {
		if (e instanceof SyntaxError) return false;
		throw e;
	}
}
