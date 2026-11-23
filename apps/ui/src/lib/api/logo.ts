import type { FirmLogo } from '$api/FirmLogo';
import { onSessionReset } from '#lib/app/session-reset.js';
import { ApiError, request, requestBlob, upload } from './client.js';

const PATH = '/settings/logo';

/** The size of the firm's logo, or null: there is none. */
export async function getLogoInfo(): Promise<FirmLogo | null> {
	try {
		return await request<FirmLogo>('GET', `${PATH}/info`);
	} catch (e) {
		if (e instanceof ApiError && e.status === 404) return null;
		throw e;
	}
}

/** The one address that is alive: the picture is shown by one screen at a time. */
let current: string | null = null;

/** Lets the address go (the screen is left, the logo is replaced or removed). */
export function releaseLogoUrl(): void {
	if (current !== null) URL.revokeObjectURL(current);
	current = null;
}

/**
 * A `blob:` address of the logo (the picture needs the token, so an `<img src>` cannot point at the API).
 * Null: there is no logo. A new address revokes the previous one.
 */
export async function logoUrl(): Promise<string | null> {
	let blob: Blob;
	try {
		blob = await requestBlob(PATH);
	} catch (e) {
		if (e instanceof ApiError && e.status === 404) {
			releaseLogoUrl();
			return null;
		}
		throw e;
	}
	releaseLogoUrl();
	current = URL.createObjectURL(blob);
	return current;
}

/** Puts a new logo (PNG, JPEG, WebP); the server rewrites it as a PNG. Needs `settings_manage`. */
export const uploadLogo = (file: Blob) => upload<FirmLogo>(PATH, file, undefined, 'PUT');

/** Removes the logo. Needs `settings_manage`. */
export const removeLogo = () => request<void>('DELETE', PATH);

onSessionReset(releaseLogoUrl);
