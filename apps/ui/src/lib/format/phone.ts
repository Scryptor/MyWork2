export function phoneDigits(input: string): string {
	return input.replace(/\D/g, '');
}

export function formatPhone(digits: string): string {
	let d = digits;
	if (d.length === 11 && (d.startsWith('7') || d.startsWith('8'))) d = d.slice(1);
	if (d.length === 10) return `+7 ${d.slice(0, 3)} ${d.slice(3, 6)}-${d.slice(6, 8)}-${d.slice(8)}`;
	if (digits.length === 6) return `${digits.slice(0, 2)}-${digits.slice(2, 4)}-${digits.slice(4)}`;
	return digits;
}
