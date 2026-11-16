const NNBSP = ' ';

/** Digits in groups of three, separated by a narrow no-break space: «1 234 567». */
export function groupDigits(digits: string): string {
	return digits.replace(/\B(?=(\d{3})+$)/g, NNBSP);
}
