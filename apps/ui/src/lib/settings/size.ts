import { stepScale, type UiScale } from '#lib/design/display.js';
import { display, prefs } from '#lib/design/prefs.svelte.js';
import { ru } from '#lib/i18n/ru.js';
import { toast } from '#lib/ui/toast/toast.svelte.js';

/** 1.1 -> «110 %». */
export function scaleLabel(scale: number): string {
	return `${Math.round(scale * 100)} %`;
}

/** Ctrl/⌘ + and −: from the scale in effect now, so «Авто» steps from what it gave. */
export function stepSize(dir: 1 | -1): void {
	const next: UiScale = stepScale(display.value.scale, dir);
	prefs.set({ scale: next });
	toast.show({ message: ru.settings.sizeToast(scaleLabel(next)) });
}

/** Ctrl/⌘ 0 and «Размер по экрану». */
export function autoSize(): void {
	prefs.set({ scale: 'auto' });
	toast.show({ message: ru.settings.sizeAutoToast(scaleLabel(display.value.scale)) });
}
