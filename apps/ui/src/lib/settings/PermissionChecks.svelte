<!-- The rights of a role as checkboxes in groups; each has «Что это?» with what it allows in the program now.
     The state is the caller's: a tick goes to `ontoggle`, the caller says what the server kept. -->
<script lang="ts">
	import type { Permission } from '$api/Permission';
	import { ru } from '#lib/i18n/ru.js';
	import Checkbox from '#lib/ui/Checkbox.svelte';
	import HelpHint from '#lib/ui/HelpHint.svelte';
	import { PERMISSION_GROUPS } from './permissions.js';

	let {
		value,
		disabled = false,
		resync = 0,
		ontoggle
	}: {
		value: readonly Permission[];
		disabled?: boolean;
		/** Raised when the caller kept the old set after a tick: the boxes are drawn again from `value`. */
		resync?: number;
		ontoggle: (permission: Permission, on: boolean) => void;
	} = $props();
</script>

{#key resync}
	<div class="groups">
		{#each PERMISSION_GROUPS as group (group.id)}
			<fieldset class="group">
				<legend>{group.label}</legend>
				{#each group.permissions as p (p)}
					<div class="item">
						<Checkbox
							label={ru.permissions[p].label}
							{disabled}
							bind:checked={() => value.includes(p), (v) => ontoggle(p, v)}
						/>
						<HelpHint title={ru.permissions[p].label} text={ru.permissions[p].hint} />
					</div>
				{/each}
			</fieldset>
		{/each}
	</div>
{/key}

<style>
	.groups {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	.group {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		min-width: 0;
		margin: 0;
		padding: var(--space-2) var(--space-3);
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
	}
	legend {
		padding: 0 var(--space-1);
		font-size: var(--text-sm);
		font-weight: 600;
		color: var(--text-2);
	}
	.item {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: var(--space-2);
		min-width: 0;
	}
</style>
