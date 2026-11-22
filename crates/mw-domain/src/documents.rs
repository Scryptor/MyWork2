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
    }
}

impl DocumentKind {
    pub const ALL: [DocumentKind; 7] = [
        Self::IntakeAct,
        Self::IssueAct,
        Self::WarrantyIntakeAct,
        Self::WarrantyIssueAct,
        Self::BlankIntakeAct,
        Self::Sticker,
        Self::Calibration,
    ];

    /// Печатается из заказа: все виды, кроме пустого акта приёма и калибровки.
    pub fn for_order(self) -> bool {
        !matches!(self, Self::BlankIntakeAct | Self::Calibration)
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
    fn only_blank_act_and_calibration_are_printed_without_an_order() {
        let without: Vec<_> = DocumentKind::ALL
            .into_iter()
            .filter(|k| !k.for_order())
            .collect();
        assert_eq!(
            without,
            [DocumentKind::BlankIntakeAct, DocumentKind::Calibration]
        );
    }
}
