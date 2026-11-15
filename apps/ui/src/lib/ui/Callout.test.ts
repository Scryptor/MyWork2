import { render, screen } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import { describe, expect, it } from 'vitest';
import Callout from './Callout.svelte';

const body = createRawSnippet(() => ({ render: () => '<span>Текст</span>' }));

describe('Callout', () => {
	it('danger is an alert', () => {
		render(Callout, { tone: 'danger', children: body });
		expect(screen.getByRole('alert')).toBeInTheDocument();
	});
	it('info is a status', () => {
		render(Callout, { tone: 'info', children: body });
		expect(screen.getByRole('status')).toBeInTheDocument();
	});
});
