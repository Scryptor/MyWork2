const ORG_FORMS = new Set([
	'ООО',
	'ОАО',
	'ЗАО',
	'ПАО',
	'АО',
	'ИП',
	'ЧП',
	'ТОО',
	'МУП',
	'ГУП',
	'ФГУП',
	'НКО',
	'ТЦ',
	'СЦ'
]);

function words(s: string): string[] {
	return s.split(/\s+/).filter((w) => w !== '');
}

function capitalize(word: string): string {
	return word
		.split('-')
		.map((p) => (p === '' ? p : p.charAt(0).toUpperCase() + p.slice(1).toLowerCase()))
		.join('-');
}

/** Old databases store names in capitals: show them in normal case. */
export function displayName(stored: string): string {
	const ws = words(stored);
	if (ws.length === 0) return '';
	const hasLower = ws.some((w) => w !== w.toUpperCase());
	if (hasLower) return ws.join(' ');
	return ws.map((w) => (ORG_FORMS.has(w) ? w : capitalize(w))).join(' ');
}

export function shortName(stored: string): string {
	const ws = words(displayName(stored));
	const first = ws[0];
	if (first === undefined) return '';
	if (ORG_FORMS.has(first) || ws.length === 1) return ws.join(' ');
	return [first, ...ws.slice(1).map((w) => w.charAt(0).toUpperCase() + '.')].join(' ');
}

export function initials(stored: string): string {
	const ws = words(stored);
	const letters = ws.slice(0, 2).map((w) => w.charAt(0).toUpperCase());
	return letters.length === 0 ? '?' : letters.join('');
}
