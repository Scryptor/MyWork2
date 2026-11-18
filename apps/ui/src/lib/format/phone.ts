export function phoneDigits(input: string): string {
	return input.replace(/\D/g, '');
}

/**
 * The key one number is matched by, as the server does it (`mw_domain::phone::phone_key`): the last 10
 * digits, so +7…, 8… and a number without the code are one; a shorter number whole.
 */
export function phoneKey(input: string): string {
	const digits = phoneDigits(input);
	return digits.length >= 10 ? digits.slice(-10) : digits;
}

export function formatPhone(digits: string): string {
	let d = digits;
	if (d.length === 11 && (d.startsWith('7') || d.startsWith('8'))) d = d.slice(1);
	if (d.length === 10) return `+7 ${d.slice(0, 3)} ${d.slice(3, 6)}-${d.slice(6, 8)}-${d.slice(8)}`;
	if (digits.length === 6) return `${digits.slice(0, 2)}-${digits.slice(2, 4)}-${digits.slice(4)}`;
	return digits;
}

/** The number to dial (`tel:` links): Russian numbers with +7, short city numbers as they are. */
export function dialNumber(digits: string): string {
	let d = digits;
	if (d.length === 11 && (d.startsWith('7') || d.startsWith('8'))) d = d.slice(1);
	return d.length === 10 ? `+7${d}` : digits;
}
