import Palette from '@lucide/svelte/icons/palette';
import type { Component } from 'svelte';
import type { Path } from '$app/types';
import { ru } from '#lib/i18n/ru.js';

export interface SettingsSection {
	id: string;
	label: string;
	icon: Component;
	/** The route of the subsection; `resolve` turns it into the address. */
	href: Extract<Path, `settings/${string}`>;
}

/** Subsections of «Настройки», in order. A new subsection is one entry here plus its route. */
export const SETTINGS_SECTIONS: readonly SettingsSection[] = [
	{ id: 'appearance', label: ru.settings.appearance, icon: Palette, href: 'settings/appearance' }
];
