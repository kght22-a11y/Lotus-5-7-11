pub mod reader;
pub mod writer;

pub use reader::{deserialize_workbook, ParseError};
pub use writer::serialize_workbook;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Cell, CellValue, Coord, Sheet, Workbook};

    #[test]
    fn test_full_workbook_roundtrip() {
        let mut wb = Workbook::new("test-wb", "Lotus Test Workspace");
        let mut sheet = Sheet::new("main", "Main");

        // Cell A1: Number 100.5
        sheet.set_cell(Cell::number(Coord::new(0, 0), 100.5).with_name("input_a"));

        // Cell B1: Number 200.0
        sheet.set_cell(Cell::number(Coord::new(1, 0), 200.0).with_name("input_b"));

        // Cell C1: Formula =A1+B1
        sheet.set_cell(Cell::formula(Coord::new(2, 0), "=A1+B1").with_name("result_sum"));

        // Cell A2: Embedded BASIC procedure
        let basic_code = "SUB Calculate()\n  LET a = GET_CELL(\"A1\")\n  SET_CELL(\"B1\", a * 2)\nEND SUB";
        sheet.set_cell(Cell::basic(Coord::new(0, 1), "sim.bas", basic_code));

        // Cell B2: Named Raw Binary Payload (contains null bytes and high bytes)
        let binary_payload: Vec<u8> = vec![0x00, 0xFF, 0x55, 0xAA, 0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01];
        sheet.set_cell(Cell::blob(Coord::new(1, 1), "kernel.bin", binary_payload.clone()));

        // Cell C26: Furthest cell in extent (Col C is 2, Row 26 is 25)
        sheet.set_cell(Cell::text(Coord::new(2, 25), "Extent Cell"));

        wb.sheets.push(sheet);

        // 1. Verify dynamic TOC calculation before serialization
        // Furthest col is 2 (C), furthest row is 25 (26)
        // TOC must be at col 3 (D), row 26 (27) -> "D27"
        let toc = wb.compute_toc();
        assert_eq!(toc.pos, Coord::new(3, 26));
        assert_eq!(toc.pos.to_a1(), "D27");

        // 2. Serialize to bytes
        let serialized = serialize_workbook(&wb);

        // Verify that the serialized format contains human-readable XML framing
        let serialized_str = String::from_utf8_lossy(&serialized);
        assert!(serialized_str.contains("<workbook id=\"test-wb\""));
        assert!(serialized_str.contains("<toc ref=\"D27\""));
        assert!(serialized_str.contains("name=\"sim.bas\""));
        assert!(serialized_str.contains("name=\"kernel.bin\""));
        assert!(serialized_str.contains("type=\"blob\" bytes=\"10\""));

        // 3. Deserialize back
        let restored = deserialize_workbook(&serialized).expect("Deserialization failed");

        assert_eq!(restored.id, "test-wb");
        assert_eq!(restored.title, "Lotus Test Workspace");
        assert_eq!(restored.sheets.len(), 1);

        let restored_sheet = &restored.sheets[0];
        assert_eq!(restored_sheet.cells.len(), 6);

        // Check Number cell
        let a1 = restored_sheet.get_cell(Coord::new(0, 0)).unwrap();
        assert_eq!(a1.name.as_deref(), Some("input_a"));
        assert_eq!(a1.value, CellValue::Number(100.5));

        // Check Formula cell
        let c1 = restored_sheet.get_cell(Coord::new(2, 0)).unwrap();
        assert_eq!(c1.name.as_deref(), Some("result_sum"));
        match &c1.value {
            CellValue::Formula { expr, .. } => assert_eq!(expr, "=A1+B1"),
            _ => panic!("Expected formula"),
        }

        // Check BASIC cell
        let a2 = restored_sheet.get_cell(Coord::new(0, 1)).unwrap();
        assert_eq!(a2.name.as_deref(), Some("sim.bas"));
        match &a2.value {
            CellValue::Basic { source } => assert_eq!(source, basic_code),
            _ => panic!("Expected basic"),
        }

        // Check Binary Blob cell (byte-for-byte exactness)
        let b2 = restored_sheet.get_cell(Coord::new(1, 1)).unwrap();
        assert_eq!(b2.name.as_deref(), Some("kernel.bin"));
        match &b2.value {
            CellValue::Blob { bytes } => assert_eq!(bytes, &binary_payload),
            _ => panic!("Expected blob"),
        }

        // 4. Test Cell-as-a-File Ejection
        let ejected = restored.eject_cell("kernel.bin").expect("Ejection failed");
        assert_eq!(ejected, binary_payload.as_slice());

        // Test BASIC source ejection
        let ejected_basic = restored.eject_cell("sim.bas").expect("Eject basic failed");
        assert_eq!(ejected_basic, basic_code.as_bytes());

        // 5. Test Dynamic TOC on restored workbook
        let restored_toc = restored.compute_toc();
        assert_eq!(restored_toc.pos.to_a1(), "D27");
    }

    #[test]
    fn test_embedded_document_roundtrip() {
        use crate::doc::Document;

        let mut wb = Workbook::new("doc-wb", "Document Workspace");
        let mut sheet = Sheet::new("main", "Main");
        sheet.set_cell(Cell::number(Coord::new(0, 0), 42.0).with_name("answer"));
        wb.sheets.push(sheet);

        let doc_body = "# Analysis\nThe calculated answer is {{cell:answer}}.";
        wb = wb.with_document(Document::new("analysis-01", "Analysis Report", doc_body));

        let serialized = serialize_workbook(&wb);
        let restored = deserialize_workbook(&serialized).expect("Deserialization failed");

        assert_eq!(restored.documents.len(), 1);
        let doc = &restored.documents[0];
        assert_eq!(doc.id, "analysis-01");
        assert_eq!(doc.title, "Analysis Report");
        assert_eq!(doc.body, doc_body);

        // Test live rendering
        let rendered = doc.render_live(&restored.sheets[0], Some(&restored));
        assert_eq!(rendered, "# Analysis\nThe calculated answer is 42.00.");

        // Test document ejection
        let ejected = restored.eject_cell("analysis-01").expect("Document ejection failed");
        assert_eq!(ejected, doc_body.as_bytes());
    }

    #[test]
    fn test_z_layer_roundtrip_and_composition() {
        let mut sheet = Sheet::new("main", "Main");
        sheet.set_cell(Cell::number(Coord::new(0, 0), 100.0));
        sheet.set_cell(Cell::text(Coord::new(1, 0), "keep in base"));
        let mut wb = Workbook::new("history-wb", "History Workspace").with_sheet(sheet);

        let z1 = wb.record_history_layer("main", vec![
            (Coord::new(0, 0), Some(Cell::number(Coord::new(0, 0), 900.0))),
            (Coord::new(1, 0), None),
            (Coord::new(2, 0), Some(Cell::blob(Coord::new(2, 0), "payload", b"x</cell>\0\xff".to_vec()))),
        ]).unwrap();
        assert_eq!(z1, 1);
        wb.record_history_layer("main", vec![
            (Coord::new(0, 0), Some(Cell::number(Coord::new(0, 0), 1200.0))),
        ]).unwrap();

        let encoded = serialize_workbook(&wb);
        let restored = deserialize_workbook(&encoded).expect("History workbook should parse");
        assert_eq!(restored, wb);

        // The base remains unchanged; historical layers compose only when selected.
        assert_eq!(restored.get_sheet("main").unwrap().get_cell(Coord::new(0, 0)).unwrap().value, CellValue::Number(100.0));
        assert_eq!(restored.sheet_at_z("main", 0).unwrap().get_cell(Coord::new(0, 0)).unwrap().value, CellValue::Number(100.0));

        let at_z1 = restored.sheet_at_z("main", 1).unwrap();
        assert_eq!(at_z1.get_cell(Coord::new(0, 0)).unwrap().value, CellValue::Number(900.0));
        assert!(at_z1.get_cell(Coord::new(1, 0)).is_none());
        assert_eq!(at_z1.get_cell(Coord::new(2, 0)).unwrap().value, CellValue::Blob { bytes: b"x</cell>\0\xff".to_vec() });

        let at_z2 = restored.sheet_at_z("main", 2).unwrap();
        assert_eq!(at_z2.get_cell(Coord::new(0, 0)).unwrap().value, CellValue::Number(1200.0));
        assert!(restored.sheet_at_z("main", 3).is_none());
    }

    #[test]
    fn test_invalid_z_layer_order_is_rejected() {
        let workbook = b"<workbook id=\"w\" title=\"w\"><sheet id=\"main\" name=\"Main\"></sheet><history><layer sheet=\"main\" z=\"2\"><cell ref=\"A1\" type=\"number\">1</cell></layer></history></workbook>";
        assert!(matches!(deserialize_workbook(workbook), Err(ParseError::InvalidHistory(_))));
    }

    #[test]
    fn appended_history_trailer_accepts_a_batch_of_consecutive_layers() {
        let sheet = Sheet::new("main", "Main");
        let base = Workbook::new("batch-wb", "Batch Updates").with_sheet(sheet);
        let mut bytes = serialize_workbook(&base);
        bytes.extend_from_slice(b"\n<history>\n");
        for z in 1..=12 {
            bytes.extend_from_slice(
                format!(
                    "  <layer sheet=\"main\" z=\"{}\"><cell ref=\"A1\" type=\"number\">{}</cell></layer>\n",
                    z, z
                )
                .as_bytes(),
            );
        }
        bytes.extend_from_slice(b"</history>\n<cursor sheet=\"main\" x=\"0\" y=\"0\" z=\"12\" />\n");

        let restored = deserialize_workbook(&bytes).expect("Appended update batch should parse");
        assert_eq!(restored.history_for_sheet("main").unwrap().max_z, 12);
        assert_eq!(restored.get_sheet("main").unwrap().cells.len(), 0);
        assert_eq!(
            restored.sheet_at_z("main", 12).unwrap().get_cell(Coord::new(0, 0)).unwrap().value,
            CellValue::Number(12.0)
        );
    }

    #[test]
    fn incomplete_appended_history_batch_is_rejected() {
        let base = Workbook::new("batch-wb", "Batch Updates").with_sheet(Sheet::new("main", "Main"));
        let mut bytes = serialize_workbook(&base);
        bytes.extend_from_slice(b"\n<history><layer sheet=\"main\" z=\"1\"><cell ref=\"A1\" type=\"number\">42</cell></layer>");
        assert!(matches!(deserialize_workbook(&bytes), Err(ParseError::UnexpectedEof)));
    }

    #[test]
    fn formula_boolean_cache_and_basic_grants_roundtrip() {
        let mut sheet = Sheet::new("main", "Main");
        sheet.set_cell(Cell::number(Coord::new(0, 0), 3.0));
        sheet.set_cell(Cell::formula(Coord::new(1, 0), "=A1>5"));
        sheet.set_cell(Cell::formula(Coord::new(2, 0), "=IF(B1,\"big\",\"small\")"));
        sheet.set_cell(Cell::basic(Coord::new(0, 1), "script", "LET x = 1")
            .with_basic_grants(vec!["A1".into(), "tax_rate".into()], vec!["B2".into()]));
        crate::calc::CalcEngine::recalculate_sheet(&mut sheet, None).unwrap();
        let wb = Workbook::new("w", "w").with_sheet(sheet);
        let restored = deserialize_workbook(&serialize_workbook(&wb)).unwrap();
        let restored_sheet = restored.primary_sheet().unwrap();
        assert!(matches!(restored_sheet.get_cell(Coord::new(1, 0)).unwrap().value, CellValue::Formula { cached_bool: Some(false), .. }));
        assert!(matches!(&restored_sheet.get_cell(Coord::new(2, 0)).unwrap().value, CellValue::Formula { cached_text: Some(v), .. } if v == "small"));
        let basic = restored_sheet.get_cell(Coord::new(0, 1)).unwrap();
        assert_eq!(basic.basic_reads, vec!["A1", "tax_rate"]);
        assert_eq!(basic.basic_writes, vec!["B2"]);
    }
}
