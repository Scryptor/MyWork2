import type { Component } from 'svelte';
import { matchesQuery } from '#lib/format/search.js';

export interface CommandItem {
	id: string;
	label: string;
	hint?: string;
	icon?: Component;
	/** Shortcut spec for `Kbd`, e.g. `Mod+P`. */
	shortcut?: string;
	keywords?: string[];
	status?: { name: string; color: string };
}

export interface CommandGroup {
	id: string;
	title: string;
	items: CommandItem[];
}

/** Items whose label and keywords contain every word of the query; an empty query keeps all. */
export function filterCommands(items: CommandItem[], query: string): CommandItem[] {
	return items.filter((i) => matchesQuery([i.label, ...(i.keywords ?? [])].join(' '), query));
}
