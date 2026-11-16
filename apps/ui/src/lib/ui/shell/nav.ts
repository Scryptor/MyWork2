import type { Component } from 'svelte';

export interface NavItem {
	id: string;
	label: string;
	icon: Component;
	href?: string;
	count?: number;
	attention?: boolean;
	/** A small «✕» after the item, such as removing a saved view; not shown on the icon rail. */
	remove?: { label: string; onclick: () => void };
}

export interface NavSection {
	title?: string;
	items: NavItem[];
}
