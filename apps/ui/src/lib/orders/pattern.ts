// The unlock pattern of a device (plan 8.2, decision 4): the points of a 3 × 3 grid numbered 1–9 left to
// right, top to bottom, as text in the order they were entered — «1596». Empty: no pattern.

/** A pattern has at least this many points: fewer is not one yet. */
export const PATTERN_MIN_POINTS = 4;

/** The points of the grid, in rows. */
export const PATTERN_POINTS = [1, 2, 3, 4, 5, 6, 7, 8, 9] as const;

/** Row and column of a point, from 0. */
const cell = (point: number) => [Math.floor((point - 1) / 3), (point - 1) % 3] as const;

/**
 * The point a straight step from `a` to `b` goes over: the middle of a row, a column or a diagonal;
 * null for a neighbour or a knight step.
 */
function between(a: number, b: number): number | null {
	const [ra, ca] = cell(a);
	const [rb, cb] = cell(b);
	if ((ra + rb) % 2 !== 0 || (ca + cb) % 2 !== 0) return null;
	const mid = ((ra + rb) / 2) * 3 + (ca + cb) / 2 + 1;
	return mid === a || mid === b ? null : mid;
}

const isPoint = (p: number) => Number.isInteger(p) && p >= 1 && p <= 9;

/**
 * The pattern with one more point. A step over a point not in the pattern yet takes it first, as on
 * Android (1 → 3 is «123»); a point already in the pattern changes nothing.
 */
export function addPoint(value: string, point: number): string {
	if (!isPoint(point) || value.includes(String(point))) return value;
	const last = Number(value.at(-1));
	const mid = isPoint(last) ? between(last, point) : null;
	const over = mid !== null && !value.includes(String(mid)) ? String(mid) : '';
	return `${value}${over}${point}`;
}

/** The pattern as text for people: «1 · 5 · 9 · 6», as in the documents; empty — empty. */
export function patternText(value: string): string {
	return [...value].join(' · ');
}

/** The pattern has enough points to be kept. */
export function patternReady(value: string): boolean {
	return value.length >= PATTERN_MIN_POINTS;
}

/** What the server takes: empty, or 4 to 9 points 1–9 without repeats. */
export function isPattern(value: string): boolean {
	if (value === '') return true;
	return /^[1-9]{4,9}$/.test(value) && new Set(value).size === value.length;
}
