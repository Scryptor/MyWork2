import type { Component } from 'svelte';

export interface NavItem {
	id: string;
	label: string;
	icon: Component;
	href?: string;
	count?: number;
	attention?: boolean;
}

export interface NavSection {
	title?: string;
	items: NavItem[];
}
