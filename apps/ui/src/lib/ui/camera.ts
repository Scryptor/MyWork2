import type { LocalDateTime } from '#lib/format/date.js';
import { ru } from '#lib/i18n/ru.js';

/** What to tell a person whose camera did not start. */
export function cameraErrorText(e: unknown): string {
	const name = typeof e === 'object' && e !== null && 'name' in e ? String(e.name) : '';
	if (name === 'NotAllowedError') return ru.camera.denied;
	if (
		name === 'NotFoundError' ||
		name === 'OverconstrainedError' ||
		typeof navigator === 'undefined' ||
		!navigator.mediaDevices
	) {
		return ru.camera.unavailable;
	}
	return ru.camera.failed;
}

const p2 = (n: number) => String(n).padStart(2, '0');

/** `camera-20260307-090501.jpg` */
export function cameraFileName(t: LocalDateTime): string {
	return `camera-${t.year}${p2(t.month)}${p2(t.day)}-${p2(t.hour)}${p2(t.minute)}${p2(t.second)}.jpg`;
}
