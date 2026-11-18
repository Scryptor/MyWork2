import { stepScale } from '#lib/design/display.js';
import { display, prefs } from '#lib/design/prefs.svelte.js';
import { ru } from '#lib/i18n/ru.js';
import { toast } from '#lib/ui/toast/toast.svelte.js';

/** 1.1 -> «110 %». */
export function scaleLabel(scale: number): string {
	return `${Math.round(scale * 100)} %`;
}

/** One size toast at a time: a new one replaces the one still on screen. */
let shown: string | null = null;
function say(message: string): void {
	if (shown !== null) toast.dismiss(shown);
	shown = toast.show({ message });
}

/**
 * Ctrl/⌘ + and −: from the scale in effect now, so «Авто» steps from what it gave. At the end the
 * preference is kept (an «Авто» stays «Авто»), and the toast says the size.
 */
export function stepSize(dir: 1 | -1): void {
	const now = display.value.scale;
	const next = stepScale(now, dir);
	if (next !== now) prefs.set({ scale: next });
	say(
		prefs.value.scale === 'auto'
			? ru.settings.sizeAutoToast(scaleLabel(now))
			: ru.settings.sizeToast(scaleLabel(next))
	);
}

/** Ctrl/⌘ 0 and «Размер по экрану». */
export function autoSize(): void {
	prefs.set({ scale: 'auto' });
	say(ru.settings.sizeAutoToast(scaleLabel(display.value.scale)));
}
