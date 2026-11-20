<script lang="ts">
	import Search from '@lucide/svelte/icons/search';
	import {
		Checkbox,
		ChipGroup,
		Combobox,
		DateInput,
		HelpHint,
		MoneyInput,
		PhoneInput,
		SegmentedControl,
		Section,
		Select,
		Switch,
		TagInput,
		TextInput,
		Textarea
	} from '#lib/ui/index.js';

	const BRANDS = [
		{ value: 'Samsung', hint: '124 заказа' },
		{ value: 'Apple', hint: '98 заказов' },
		{ value: 'Xiaomi' },
		{ value: 'Ёмкость-тест' }
	];
	const KINDS = [
		{ value: 'phone', label: 'Смартфон' },
		{ value: 'tablet', label: 'Планшет' },
		{ value: 'laptop', label: 'Ноутбук' }
	];
	const KIT = ['Чехол', 'Зарядка', 'Коробка', 'SIM-карта'];
	const URGENCY = [
		{ value: 'normal', label: 'Обычный' },
		{ value: 'urgent', label: 'Срочно' },
		{ value: 'express', label: 'Экспресс' }
	];

	let text = $state('');
	let textHint = $state('');
	let textError = $state('Иванов');
	let textOff = $state('Недоступно');
	let textSm = $state('');
	let password = $state('secret');
	let search = $state('');
	let area = $state('');
	let money = $state<number | null>(650000);
	let moneyHint = $state<number | null>(null);
	let moneyErr = $state<number | null>(null);
	let moneyOff = $state<number | null>(120050);
	let moneySm = $state<number | null>(null);
	let phone = $state('70000000001');
	let phoneErr = $state('');
	let phoneOff = $state('70000000002');
	let phoneSm = $state('');
	let agree = $state(true);
	let partial = $state(false);
	let off = $state(false);
	let sw = $state(true);
	let swOff = $state(false);
	let seg = $state<'normal' | 'urgent' | 'express'>('normal');
	let segSm = $state<'normal' | 'urgent' | 'express'>('urgent');
	let kit = $state<string[]>(['Чехол']);
	let helpText = $state('');
	let helpSwitch = $state(false);
	let kind = $state<string | null>(null);
	let kindHint = $state<string | null>('tablet');
	let kindErr = $state<string | null>(null);
	let kindOff = $state<string | null>('phone');
	let kindSm = $state<string | null>(null);
	/** Options in groups, as the time zones of «Общие». */
	const ZONES = [
		{
			label: 'Россия',
			options: [
				{ value: 'Europe/Kaliningrad', label: 'Калининград (UTC+02:00)' },
				{ value: 'Europe/Moscow', label: 'Москва (UTC+03:00)' },
				{ value: 'Europe/Samara', label: 'Самара (UTC+04:00)' }
			]
		},
		{
			label: 'Европа',
			options: [
				{
					value: 'Europe/Berlin',
					label: 'Центральная Европа, стандартное время (UTC+01:00) — Europe/Berlin'
				}
			]
		}
	];
	let zone = $state<string | null>('Europe/Moscow');
	let combo = $state('');
	let comboErr = $state('');
	let comboOff = $state('Apple');
	let comboAll = $state('');
	let comboSm = $state('');
	let date = $state<string | null>(null);
	let dateErr = $state<string | null>(null);
	let dateOff = $state<string | null>('2026-11-15');
	let dateSm = $state<string | null>(null);

	let masters = $state<string[]>(['Петров', 'Сидоров']);
	let mastersErr = $state<string[]>([]);

	let brand = $state('');
	let kitReception = $state<string[]>([]);
	let prepay = $state<number | null>(null);
	let phoneReception = $state('');
	let dateReception = $state<string | null>(null);
</script>

<h3>Текст</h3>
<div class="grid">
	<TextInput bind:value={text} label="Обычное" placeholder="Введите текст" />
	<TextInput bind:value={textHint} label="С подсказкой" hint="Как в паспорте" required />
	<TextInput bind:value={textError} label="С ошибкой" error="Такой клиент уже есть" />
	<TextInput bind:value={textOff} label="Отключённое" disabled />
	<TextInput bind:value={textSm} label="Компактное" size="sm" />
	<TextInput bind:value={password} label="Пароль" type="password" revealable />
	<TextInput bind:value={search} label="Поиск" type="search" placeholder="Поиск">
		{#snippet prefix()}<Search size={16} aria-hidden="true" />{/snippet}
	</TextInput>
	<Textarea bind:value={area} label="Многострочное" hint="Растёт по содержимому" />
</div>

<h3>Деньги и телефон</h3>
<div class="grid">
	<MoneyInput bind:value={money} label="Сумма" />
	<MoneyInput bind:value={moneyHint} label="Сумма с подсказкой" hint="Копейки через запятую" />
	<MoneyInput bind:value={moneyErr} label="Сумма с ошибкой" error="Сумма обязательна" />
	<MoneyInput bind:value={moneyOff} label="Сумма отключённая" disabled />
	<MoneyInput bind:value={moneySm} label="Сумма без пустого" nullable={false} />
	<PhoneInput bind:value={phone} label="Телефон" />
	<PhoneInput bind:value={phoneErr} label="Телефон с ошибкой" error="Неверный номер" />
	<PhoneInput bind:value={phoneOff} label="Телефон отключённый" disabled />
	<PhoneInput bind:value={phoneSm} label="Телефон с подсказкой" hint="Любой формат" />
</div>

<h3>Переключатели</h3>
<div class="grid">
	<Checkbox bind:checked={agree} label="Согласен" hint="С условиями гарантии" />
	<Checkbox bind:checked={partial} label="Частично" indeterminate />
	<Checkbox bind:checked={off} label="Отключён" disabled />
	<Switch bind:checked={sw} label="Уведомлять клиента" hint="СМС при готовности" />
	<Switch bind:checked={swOff} label="Отключён" disabled />
	<SegmentedControl bind:value={seg} options={URGENCY} label="Срочность" />
	<SegmentedControl bind:value={segSm} options={URGENCY} label="Срочность, компактно" size="sm" />
</div>

<h3>Выбор</h3>
<div class="grid">
	<Select bind:value={kind} items={KINDS} label="Тип устройства" />
	<Select bind:value={kindHint} items={KINDS} label="Выбор с подсказкой" hint="Из справочника" />
	<Select bind:value={kindErr} items={KINDS} label="Выбор с ошибкой" error="Укажите тип" />
	<Select bind:value={kindOff} items={KINDS} label="Выбор отключённый" disabled />
	<Select bind:value={zone} items={ZONES} label="Выбор с группами" />
	<Select bind:value={kindSm} items={KINDS} label="Выбор без значения" placeholder="Не выбрано" />
	<Combobox bind:value={combo} items={BRANDS} label="Подсказки" placeholder="Начните вводить" />
	<Combobox bind:value={comboErr} items={BRANDS} label="Подсказки с ошибкой" error="Нужен бренд" />
	<Combobox bind:value={comboOff} items={BRANDS} label="Подсказки отключённые" disabled />
	<Combobox
		bind:value={comboAll}
		items={BRANDS}
		label="Без фильтра (список от сервера)"
		filter={false}
	/>
	<Combobox bind:value={comboSm} items={BRANDS} label="Только из списка" allowCustom={false} />
	<DateInput bind:value={date} label="Дата" />
	<DateInput bind:value={dateErr} label="Дата с ошибкой" error="Дата обязательна" />
	<DateInput bind:value={dateOff} label="Дата отключённая" disabled />
	<DateInput bind:value={dateSm} label="Дата с подсказкой" hint="дд.мм.гггг" min="2026-01-01" />
</div>

<h3>Список значений</h3>
<div class="grid">
	<TagInput bind:values={masters} label="Мастера" hint="Имя и Enter" />
	<TagInput bind:values={mastersErr} label="Мастера с ошибкой" error="Добавьте хотя бы одного" />
</div>

<h3>Подсказки «Что это?»</h3>
<div class="grid">
	<TextInput
		bind:value={helpText}
		label="Долгая диагностика, дней"
		help={[
			'Сколько суток заказ может быть на диагностике, прежде чем попасть в «Долгая диагностика».',
			'Подсказка может быть из нескольких абзацев.'
		]}
	/>
	<Switch
		bind:checked={helpSwitch}
		label="Проверять обязательные поля"
		help="Значок справа от подписи переключателя."
	/>
	<Section title="Раздел с подсказкой" help="Значок справа от заголовка раздела.">
		<p class="muted">Содержимое раздела</p>
	</Section>
	<HelpHint title="Отдельный значок" text="Значок можно поставить рядом с любой подписью." />
</div>

<h3>Приём устройства</h3>
<div class="grid">
	<Combobox bind:value={brand} items={BRANDS} label="Бренд" placeholder="Samsung, Apple…" />
	<MoneyInput bind:value={prepay} label="Предоплата" />
	<PhoneInput bind:value={phoneReception} label="Телефон клиента" />
	<DateInput bind:value={dateReception} label="Дата приёма" />
</div>
<ChipGroup bind:selected={kitReception} options={KIT} label="Комплектность" allowCustom />
<p class="muted">Выбрано: {kit.join(', ') || 'ничего'}</p>
<ChipGroup bind:selected={kit} options={KIT} label="Комплектность, пример" />

<style>
	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(16rem, 1fr));
		gap: var(--space-4);
		align-items: start;
		margin-bottom: var(--space-4);
	}
	.muted {
		color: var(--text-3);
	}
</style>
