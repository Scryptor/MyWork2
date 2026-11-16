export const ru = {
	common: {
		close: 'Закрыть',
		cancel: 'Отмена',
		undo: 'Отменить',
		retry: 'Повторить',
		more: 'Ещё',
		loading: 'Загрузка…',
		nothingFound: 'Ничего не найдено',
		back: 'Назад'
	},
	flags: { needCall: 'Позвонить', needParts: 'Заказать запчасть', blacklisted: 'Чёрный список' },
	fields: {
		moneyError: 'Введите сумму, например 6 500',
		custom: '+ своё',
		select: 'Выберите',
		openCalendar: 'Открыть календарь',
		datePlaceholder: 'дд.мм.гггг',
		dateError: 'Введите дату, например 15.11.2026',
		dateRangeError: 'Дата вне допустимого диапазона',
		prevMonth: 'Предыдущий месяц',
		nextMonth: 'Следующий месяц',
		showPassword: 'Показать пароль',
		hidePassword: 'Скрыть пароль'
	},
	save: { saving: 'Сохраняю…', saved: 'Сохранено', failed: 'Не сохранено' },
	panel: { prev: 'Предыдущая запись', next: 'Следующая запись' },
	toast: { region: 'Уведомления' },
	shell: {
		nav: 'Разделы',
		bottomNav: 'Основная навигация',
		attention: 'Требует внимания'
	},
	palette: {
		title: 'Найти или выполнить',
		placeholder: 'Найти заказ, клиента или команду…',
		empty: 'Ничего не найдено'
	},
	shortcuts: { title: 'Клавиши' },
	media: {
		photos: 'Фото устройства',
		addPhoto: 'Добавить фото',
		removePhoto: 'Удалить фото',
		of: 'из',
		files: 'Файлы',
		chooseFiles: 'Выбрать файлы',
		dropHint: 'Перетащите файлы сюда или вставьте из буфера'
	},
	status: { label: 'Статус' },
	table: {
		columns: 'Колонки',
		moveUp: 'Выше',
		moveDown: 'Ниже',
		selectAll: 'Выбрать все видимые',
		selectRow: 'Выбрать строку',
		selected: 'Выбрано',
		selection: 'Выбранные строки',
		clearSelection: 'Снять выделение',
		empty: 'Нет записей'
	}
} as const;
