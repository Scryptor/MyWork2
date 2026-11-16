<script lang="ts">
	import ChevronDown from '@lucide/svelte/icons/chevron-down';
	import Copy from '@lucide/svelte/icons/copy';
	import Info from '@lucide/svelte/icons/info';
	import Printer from '@lucide/svelte/icons/printer';
	import Trash2 from '@lucide/svelte/icons/trash-2';
	import { matchShortcut } from '#lib/design/hotkeys.svelte.js';
	import { displayName } from '#lib/format/name.js';
	import {
		Button,
		ConfirmDialog,
		Dialog,
		DropdownMenu,
		Kbd,
		Popover,
		Select,
		Sheet,
		SlideOver,
		StatusBadge,
		TextInput,
		Textarea,
		toast,
		type MenuItem
	} from '#lib/ui/index.js';

	const ORDER = '№ 10481';
	const RECORDS = 5;
	const MASTERS = ['ИВАНОВ ИВАН', 'ПЕТРОВА АННА', 'СИДОРОВ ПЁТР'].map((n) => ({
		value: n,
		label: displayName(n)
	}));
	const STATUSES = {
		diagnostics: { name: 'Диагностика', color: 'amber' },
		ready: { name: 'Готов', color: 'green' }
	} as const;

	let picked = $state('');
	const MENU: MenuItem[] = [
		{ label: 'Печать', icon: Printer, shortcut: 'Mod+P', onselect: () => (picked = 'Печать') },
		{ label: 'Дублировать', icon: Copy, onselect: () => (picked = 'Дублировать') },
		{ label: 'Вернуть в работу', disabled: true, onselect: () => (picked = 'Вернуть в работу') },
		{ separator: true },
		{ label: 'Удалить', icon: Trash2, danger: true, onselect: () => (picked = 'Удалить') }
	];

	let dialogOpen = $state(false);
	let comment = $state('');
	let confirmOpen = $state(false);
	let removed = $state(0);

	let panelOpen = $state(false);
	let record = $state(1);
	let master = $state<string | null>(null);
	let note = $state('');
	let clicks = $state(0);

	let sheetOpen = $state(false);
	let intakePhone = $state('');
	let problem = $state('');
	let accepted = $state(0);

	let status = $state<keyof typeof STATUSES>('diagnostics');

	async function remove() {
		await new Promise((resolve) => setTimeout(resolve, 300));
		removed++;
	}

	function accept() {
		accepted++;
		sheetOpen = false;
		intakePhone = '';
		problem = '';
	}

	function toReady() {
		if (status === 'ready') return;
		const previous = status;
		status = 'ready';
		toast.undoable(`${ORDER} → ${STATUSES.ready.name}`, () => {
			status = previous;
		});
	}
</script>

<h3>Всплывающие</h3>
<div class="row">
	<Popover>
		{#snippet trigger(props)}
			<button {...props} type="button" class="trigger">
				<Info size={16} aria-hidden="true" />Подробнее
			</button>
		{/snippet}
		<p class="pop">Гарантия 30 дней с даты выдачи.</p>
	</Popover>
	<DropdownMenu items={MENU} label="Действия с заказом">
		{#snippet trigger(props)}
			<button {...props} type="button" class="trigger">
				Действия<ChevronDown size={16} aria-hidden="true" />
			</button>
		{/snippet}
	</DropdownMenu>
	<span class="muted">Выбрано: {picked || 'ничего'}</span>
</div>

<h3>Диалоги</h3>
<div class="row">
	<Button onclick={() => (dialogOpen = true)}>Открыть диалог</Button>
	<Button variant="danger" icon={Trash2} onclick={() => (confirmOpen = true)}>
		Удалить навсегда
	</Button>
	<span class="muted">Удалено: {removed}</span>
</div>
<Dialog
	bind:open={dialogOpen}
	title="Комментарий к заказу"
	description="Виден только сотрудникам сервиса."
>
	<Textarea bind:value={comment} label="Комментарий" />
	{#snippet footer()}
		<Button onclick={() => (dialogOpen = false)}>Отмена</Button>
		<Button variant="primary" onclick={() => (dialogOpen = false)}>Сохранить</Button>
	{/snippet}
</Dialog>
<ConfirmDialog
	bind:open={confirmOpen}
	title="Удалить заказ {ORDER} навсегда"
	description="Заказ, его история и фото будут удалены. Вернуть их будет нельзя."
	confirmLabel="Удалить"
	onconfirm={remove}
/>

<h3>Панели</h3>
<div class="row">
	<Button onclick={() => (panelOpen = true)}>Открыть панель</Button>
	<Button onclick={() => clicks++}>Счётчик: {clicks}</Button>
	<Button onclick={() => (sheetOpen = true)}>Открыть широкую панель</Button>
	<span class="muted">Принято: {accepted}</span>
</div>
<SlideOver
	bind:open={panelOpen}
	title="Запись № {record}"
	onprev={record > 1 ? () => record-- : undefined}
	onnext={record < RECORDS ? () => record++ : undefined}
>
	{#snippet header()}
		<p class="muted">
			Вымышленная запись {record} из {RECORDS}. Соседняя — ‹ ›, ↑↓ вне поля или
			<Kbd shortcut="Alt+ArrowDown" />
		</p>
	{/snippet}
	<div class="stack">
		<Select bind:value={master} items={MASTERS} label="Мастер" />
		<TextInput bind:value={note} label="Заметка" />
	</div>
	{#snippet footer()}
		<Button variant="primary" onclick={() => (panelOpen = false)}>Готово</Button>
	{/snippet}
</SlideOver>
<Sheet bind:open={sheetOpen} title="Новый заказ">
	<!-- Accepting is Ctrl/⌘+Enter only: plain Enter in «Телефон» must not submit the form.
	     The shortcut belongs to the form's fields; the form itself is not a control. -->
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<form
		class="stack"
		onsubmit={(e) => e.preventDefault()}
		onkeydown={(e) => {
			if (!matchShortcut(e, 'Mod+Enter')) return;
			e.preventDefault();
			accept();
		}}
	>
		<TextInput bind:value={intakePhone} label="Телефон" type="tel" />
		<Textarea bind:value={problem} label="Неисправность" />
	</form>
	{#snippet footer()}
		<Button onclick={() => (sheetOpen = false)}>Отмена</Button>
		<Button variant="primary" shortcut="Mod+Enter" onclick={accept}>Принять</Button>
	{/snippet}
</Sheet>

<h3>Уведомления</h3>
<p>
	Статус заказа {ORDER}:
	<StatusBadge name={STATUSES[status].name} color={STATUSES[status].color} />
</p>
<div class="row">
	<Button onclick={toReady}>Статус → Готов</Button>
	<Button onclick={() => toast.show({ message: 'Отправлено на принтер «Приёмка»' })}>
		Уведомление
	</Button>
	<Button onclick={() => toast.show({ message: 'Сохранено', tone: 'success' })}>Успех</Button>
	<Button
		onclick={() =>
			toast.show({
				message: 'Не удалось сохранить: нет связи с сервером',
				tone: 'danger',
				action: {
					label: 'Повторить',
					run: () => {
						toast.show({ message: 'Сохранено', tone: 'success' });
					}
				}
			})}
	>
		Ошибка
	</Button>
	<span class="muted">Отменить последнее вне поля: <Kbd shortcut="Mod+Z" /></span>
</div>

<style>
	.row {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--space-2);
		margin-bottom: var(--space-3);
	}
	.stack {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}
	.muted {
		margin: 0;
		color: var(--text-3);
	}
	.pop {
		margin: 0;
		max-width: 16rem;
	}
	.trigger {
		display: inline-flex;
		align-items: center;
		gap: var(--space-2);
		box-sizing: border-box;
		height: var(--control-md);
		padding: 0 var(--space-3);
		border: 1px solid var(--border-strong);
		border-radius: var(--radius-md);
		background: var(--surface);
		color: var(--text);
		font: inherit;
		font-size: var(--text-md);
		font-weight: 500;
		cursor: pointer;
	}
	.trigger:hover {
		background: var(--surface-3);
	}
	@media (pointer: coarse) {
		.trigger {
			min-height: 2.75rem;
		}
	}
</style>
