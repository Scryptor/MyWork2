export type ShellLayout = 'phone' | 'rail' | 'full';

export const PHONE_QUERY = '(max-width: 599px)';
export const RAIL_QUERY = '(min-width: 600px) and (max-width: 1279px)';
/** The full order card has two columns from this width; narrower it has tabs (spec 6.3). */
export const CARD_COLUMNS_QUERY = '(min-width: 1024px)';

export function layoutForWidth(width: number): ShellLayout {
	if (width < 600) return 'phone';
	if (width < 1280) return 'rail';
	return 'full';
}
