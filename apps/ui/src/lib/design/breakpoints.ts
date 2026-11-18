export type ShellLayout = 'phone' | 'rail' | 'full';

export const PHONE_QUERY = '(max-width: 599px)';
export const RAIL_QUERY = '(min-width: 600px) and (max-width: 1279px)';
/** The full order card has two columns from this width; narrower it has tabs (spec 6.3). */
export const CARD_COLUMNS_QUERY = '(min-width: 1024px)';

/** Lower edges of the width bands, in layout pixels (window width / scale). */
export const BAND_LIMITS = { xs: 600, sm: 900, md: 1024, lg: 1280 } as const;

export function layoutForWidth(width: number): ShellLayout {
	if (width < BAND_LIMITS.xs) return 'phone';
	if (width < BAND_LIMITS.lg) return 'rail';
	return 'full';
}
