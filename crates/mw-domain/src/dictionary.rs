code_enum! {
    /// Вид справочника.
    DictionaryKind {
        DeviceType => "device_type",
        Brand => "brand",
        Color => "color",
        Completeness => "completeness",
        Condition => "condition",
        Malfunction => "malfunction",
        WorkDone => "work_done",
        Master => "master",
        ClientSource => "client_source",
        Warranty => "warranty",
    }
}

impl DictionaryKind {
    /// Название справочника для людей.
    pub fn label_ru(self) -> &'static str {
        match self {
            Self::DeviceType => "Типы устройств",
            Self::Brand => "Бренды",
            Self::Color => "Цвета",
            Self::Completeness => "Комплектность",
            Self::Condition => "Состояние",
            Self::Malfunction => "Неисправности",
            Self::WorkDone => "Выполненные работы",
            Self::Master => "Мастера",
            Self::ClientSource => "Откуда узнали",
            Self::Warranty => "Гарантия",
        }
    }
}

/// Значения справочников по умолчанию (справочник мастеров заполняется пользователями).
pub const DEFAULT_DICTIONARY: &[(DictionaryKind, &[&str])] = &[
    (
        DictionaryKind::DeviceType,
        &[
            "Телефон",
            "Планшет",
            "Ноутбук",
            "Компьютер",
            "Телевизор",
            "Игровая приставка",
            "Часы",
            "Наушники",
        ],
    ),
    (
        DictionaryKind::Brand,
        &[
            "Apple", "Samsung", "Xiaomi", "Huawei", "Honor", "Realme", "Lenovo", "Asus", "HP",
            "Sony",
        ],
    ),
    (
        DictionaryKind::Color,
        &[
            "Чёрный",
            "Белый",
            "Серый",
            "Серебристый",
            "Золотой",
            "Синий",
            "Красный",
            "Зелёный",
        ],
    ),
    (
        DictionaryKind::Completeness,
        &[
            "Устройство",
            "Чехол",
            "Зарядка",
            "Кабель",
            "Сим-карта",
            "Карта памяти",
            "Коробка",
        ],
    ),
    (
        DictionaryKind::Condition,
        &[
            "Без видимых повреждений",
            "Царапины",
            "Сколы",
            "Трещина экрана",
            "Следы влаги",
        ],
    ),
    (
        DictionaryKind::Malfunction,
        &[
            "Не включается",
            "Не заряжается",
            "Разбит экран",
            "Залит",
            "Нет звука",
            "Не ловит сеть",
            "Быстро разряжается",
        ],
    ),
    (
        DictionaryKind::WorkDone,
        &[
            "Диагностика",
            "Замена дисплея",
            "Замена аккумулятора",
            "Замена разъёма зарядки",
            "Чистка",
            "Прошивка",
        ],
    ),
    (DictionaryKind::Master, &[]),
    (
        DictionaryKind::ClientSource,
        &[
            "Рекомендация",
            "Интернет",
            "Авито",
            "Вывеска",
            "Повторное обращение",
        ],
    ),
    (
        DictionaryKind::Warranty,
        &[
            "Без гарантии",
            "14 дней",
            "30 дней",
            "3 месяца",
            "6 месяцев",
            "1 год",
        ],
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dictionary_kind_labels_are_russian() {
        let labels: Vec<_> = [
            DictionaryKind::DeviceType,
            DictionaryKind::Brand,
            DictionaryKind::Color,
            DictionaryKind::Completeness,
            DictionaryKind::Condition,
            DictionaryKind::Malfunction,
            DictionaryKind::WorkDone,
            DictionaryKind::Master,
            DictionaryKind::ClientSource,
            DictionaryKind::Warranty,
        ]
        .map(DictionaryKind::label_ru)
        .to_vec();
        assert_eq!(
            labels,
            [
                "Типы устройств",
                "Бренды",
                "Цвета",
                "Комплектность",
                "Состояние",
                "Неисправности",
                "Выполненные работы",
                "Мастера",
                "Откуда узнали",
                "Гарантия"
            ]
        );
    }

    #[test]
    fn dictionary_kinds_roundtrip_and_defaults_are_unique() {
        for (kind, values) in DEFAULT_DICTIONARY {
            assert_eq!(kind.as_str().parse::<DictionaryKind>().unwrap(), *kind);
            let mut v = values.to_vec();
            v.sort();
            v.dedup();
            assert_eq!(v.len(), values.len(), "{kind:?}");
        }
    }
}
