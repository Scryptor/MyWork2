export const MAX_MONEY_KOPECKS = 99_999_999_999;

const NNBSP = ' ';
const NBSP = ' ';

function group(digits: string): string {
	return digits.replace(/\B(?=(\d{3})+$)/g, NNBSP);
}

export function formatMoney(kopecks: number, opts?: { currency?: string | null }): string {
	const currency = opts?.currency === undefined ? '₽' : opts.currency;
	const negative = kopecks < 0;
	const abs = Math.abs(Math.trunc(kopecks));
	const whole = Math.floor(abs / 100);
	const frac = abs % 100;
	let text = group(String(whole));
	if (frac !== 0) text += ',' + String(frac).padStart(2, '0');
	if (negative) text = '−' + text;
	return currency ? text + NBSP + currency : text;
}

export function parseMoney(input: string): number | null {
	const s = input.replace(/\s/g, ''); // \s covers NBSP and NNBSP
	const m = /^(\d+)(?:[.,](\d{1,2}))?$/.exec(s);
	if (!m) return null;
	const whole = Number(m[1]);
	if (!Number.isSafeInteger(whole) || whole > MAX_MONEY_KOPECKS) return null;
	const frac = Number((m[2] ?? '').padEnd(2, '0') || '0');
	const kopecks = whole * 100 + frac;
	return kopecks > MAX_MONEY_KOPECKS ? null : kopecks;
}
