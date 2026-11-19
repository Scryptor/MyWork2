const UNITS = [
	{ name: 'КБ', size: 1024 },
	{ name: 'МБ', size: 1024 ** 2 },
	{ name: 'ГБ', size: 1024 ** 3 }
] as const;

/** «900 Б», «1,5 КБ», «1,2 МБ», «3,5 ГБ»: sizes of files for people. */
export function formatBytes(n: number): string {
	if (n < 1024) return `${n} Б`;
	const unit = UNITS.findLast((u) => n >= u.size) ?? UNITS[0];
	const text = (Math.round((n / unit.size) * 10) / 10).toString().replace('.', ',');
	return `${text} ${unit.name}`;
}
