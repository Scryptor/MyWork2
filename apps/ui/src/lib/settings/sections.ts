import BookOpen from '@lucide/svelte/icons/book-open';
import Info from '@lucide/svelte/icons/info';
import ListOrdered from '@lucide/svelte/icons/list-ordered';
import Palette from '@lucide/svelte/icons/palette';
import Settings2 from '@lucide/svelte/icons/settings-2';
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
	{ id: 'general', label: ru.settings.general, icon: Settings2, href: 'settings/general' },
	{ id: 'appearance', label: ru.settings.appearance, icon: Palette, href: 'settings/appearance' },
	{ id: 'orders', label: ru.settings.ordersTitle, icon: ListOrdered, href: 'settings/orders' },
	{
		id: 'dictionaries',
		label: ru.settings.dictionaries,
		icon: BookOpen,
		href: 'settings/dictionaries'
	},
	{ id: 'about', label: ru.settings.about, icon: Info, href: 'settings/about' }
];
