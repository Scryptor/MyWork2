export type PluralForms = readonly [one: string, few: string, many: string];

const rules = new Intl.PluralRules('ru');

export function plural(n: number, forms: PluralForms): string {
	switch (rules.select(n)) {
		case 'one':
			return forms[0];
		case 'few':
			return forms[1];
		case 'many':
			return forms[2];
		default:
			return forms[1];
	}
}

export function countLabel(n: number, forms: PluralForms): string {
	const sign = n < 0 ? '−' : '';
	const digits = String(Math.abs(Math.trunc(n))).replace(/\B(?=(\d{3})+$)/g, ' ');
	return `${sign}${digits} ${plural(n, forms)}`;
}
