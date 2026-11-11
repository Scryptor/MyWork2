code_enum! {
    /// Вид статуса: от него зависят правила заказа.
    StatusKind {
        Open => "open",
        Diagnostics => "diagnostics",
        Ready => "ready",
        Issued => "issued",
    }
}

/// Стандартный статус, создаваемый при первоначальном заполнении БД.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefaultStatus {
    pub name: &'static str,
    pub kind: StatusKind,
    pub color: &'static str,
}

const fn st(name: &'static str, kind: StatusKind, color: &'static str) -> DefaultStatus {
    DefaultStatus { name, kind, color }
}

pub const DEFAULT_STATUSES: [DefaultStatus; 8] = [
    st("Диагностика", StatusKind::Diagnostics, "amber"),
    st("Согласование с клиентом", StatusKind::Open, "violet"),
    st("Согласовано", StatusKind::Open, "indigo"),
    st("Принят в работу", StatusKind::Open, "blue"),
    st("Ждёт запчасть", StatusKind::Open, "red"),
    st("Принят по гарантии", StatusKind::Open, "teal"),
    st("Готов", StatusKind::Ready, "green"),
    st("Выдан", StatusKind::Issued, "gray"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_statuses_match_mywork2_names_and_kinds() {
        let names: Vec<_> = DEFAULT_STATUSES.iter().map(|s| s.name).collect();
        assert_eq!(
            names,
            [
                "Диагностика",
                "Согласование с клиентом",
                "Согласовано",
                "Принят в работу",
                "Ждёт запчасть",
                "Принят по гарантии",
                "Готов",
                "Выдан"
            ]
        );
        assert_eq!(DEFAULT_STATUSES[0].kind, StatusKind::Diagnostics);
        assert_eq!(DEFAULT_STATUSES[6].kind, StatusKind::Ready);
        assert_eq!(DEFAULT_STATUSES[7].kind, StatusKind::Issued);
        assert!(
            DEFAULT_STATUSES[1..6]
                .iter()
                .all(|s| s.kind == StatusKind::Open)
        );
    }
}
