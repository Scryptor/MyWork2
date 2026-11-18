export type ShellLayout = 'phone' | 'rail' | 'full';

/** Lower edges of the width bands, in layout pixels (window width / scale). */
export const BAND_LIMITS = { xs: 600, sm: 900, md: 1024, lg: 1280 } as const;
