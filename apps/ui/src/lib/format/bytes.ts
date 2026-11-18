/** «900 Б», «1,5 КБ», «1,2 МБ»: sizes of files for people. */
export function formatBytes(n: number): string {
	if (n < 1024) return `${n} Б`;
	const unit = n < 1024 * 1024 ? 'КБ' : 'МБ';
	const value = n / (unit === 'КБ' ? 1024 : 1024 * 1024);
	const text = (Math.round(value * 10) / 10).toString().replace('.', ',');
	return `${text} ${unit}`;
}
