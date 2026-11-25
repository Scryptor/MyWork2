//! Печатные документы: акты, наклейка, калибровочная этикетка.

code_enum! {
    /// Вид документа.
    DocumentKind {
        IntakeAct => "intake_act",
        IssueAct => "issue_act",
        WarrantyIntakeAct => "warranty_intake_act",
        WarrantyIssueAct => "warranty_issue_act",
        BlankIntakeAct => "blank_intake_act",
        Sticker => "sticker",
        Calibration => "calibration",
        CellSticker => "cell_sticker",
    }
}

impl DocumentKind {
    pub const ALL: [DocumentKind; 8] = [
        Self::IntakeAct,
        Self::IssueAct,
        Self::WarrantyIntakeAct,
        Self::WarrantyIssueAct,
        Self::BlankIntakeAct,
        Self::Sticker,
        Self::Calibration,
        Self::CellSticker,
    ];

    /// Печатается из заказа: все виды, кроме пустого акта приёма, калибровки и наклейки ячейки.
    pub fn for_order(self) -> bool {
        !matches!(
            self,
            Self::BlankIntakeAct | Self::Calibration | Self::CellSticker
        )
    }

    /// Этикетка: наклейка, пробная наклейка, наклейка ячейки.
    pub fn is_label(self) -> bool {
        matches!(self, Self::Sticker | Self::Calibration | Self::CellSticker)
    }

    /// Шаблон можно настраивать: все виды, кроме калибровки.
    pub fn editable(self) -> bool {
        self != Self::Calibration
    }

    /// Позиция в [`Self::ALL`], начиная с 1.
    pub fn ordinal(self) -> i64 {
        Self::ALL
            .iter()
            .position(|k| *k == self)
            .map_or(0, |p| p as i64 + 1)
    }

    /// Название для человека.
    pub fn label_ru(self) -> &'static str {
        match self {
            Self::IntakeAct => "Акт приёма",
            Self::IssueAct => "Акт выдачи",
            Self::WarrantyIntakeAct => "Акт приёма по гарантии",
            Self::WarrantyIssueAct => "Акт выдачи после гарантии",
            Self::BlankIntakeAct => "Пустой акт приёма",
            Self::Sticker => "Наклейка",
            Self::Calibration => "Пробная наклейка",
            Self::CellSticker => "Наклейка ячейки",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_kind_codes_roundtrip() {
        for kind in DocumentKind::ALL {
            assert_eq!(kind.as_str().parse::<DocumentKind>(), Ok(kind));
            assert_eq!(
                serde_json::to_string(&kind).unwrap(),
                format!("\"{}\"", kind.as_str())
            );
        }
        assert_eq!(
            serde_json::from_str::<DocumentKind>("\"warranty_issue_act\"").unwrap(),
            DocumentKind::WarrantyIssueAct
        );
        assert!("act".parse::<DocumentKind>().is_err());
    }

    #[test]
    fn editable_ordinal_and_labels() {
        let not_editable: Vec<_> = DocumentKind::ALL
            .into_iter()
            .filter(|k| !k.editable())
            .collect();
        assert_eq!(not_editable, [DocumentKind::Calibration]);
        let ordinals: Vec<_> = DocumentKind::ALL
            .into_iter()
            .map(DocumentKind::ordinal)
            .collect();
        assert_eq!(ordinals, [1, 2, 3, 4, 5, 6, 7, 8]);
        let labels: Vec<_> = DocumentKind::ALL
            .into_iter()
            .map(DocumentKind::label_ru)
            .collect();
        assert_eq!(
            labels,
            [
                "Акт приёма",
                "Акт выдачи",
                "Акт приёма по гарантии",
                "Акт выдачи после гарантии",
                "Пустой акт приёма",
                "Наклейка",
                "Пробная наклейка",
                "Наклейка ячейки",
            ]
        );
    }

    #[test]
    fn kinds_without_an_order() {
        let without: Vec<_> = DocumentKind::ALL
            .into_iter()
            .filter(|k| !k.for_order())
            .collect();
        assert_eq!(
            without,
            [
                DocumentKind::BlankIntakeAct,
                DocumentKind::Calibration,
                DocumentKind::CellSticker
            ]
        );
    }
}
