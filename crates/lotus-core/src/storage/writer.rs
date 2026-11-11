//! Serialization of Lotus 5-7-11 workbooks to the uncompressed hybrid format.

#[cfg(not(feature = "std"))]
use alloc::{format, string::String, vec::Vec};

use crate::model::{Cell, CellValue, Workbook};

/// Serializes a `Workbook` into a `.lotus` byte buffer.
///
/// The resulting format is:
/// - Human-readable in Notepad.
/// - Clearly marked with ASCII tags in a hex editor.
/// - Directly inspectable and editable by an AI as structured text.
/// - Preserves raw, uncompressed binary payloads byte-for-byte in `<cell type="blob">` tags.
pub fn serialize_workbook(wb: &Workbook) -> Vec<u8> {
    let mut out = Vec::new();

    // 1. Header
    out.extend_from_slice(b"<!-- LOTUS-5-7-11 WORKBOOK v1.0 -->\n");
    let wb_tag = format!(
        "<workbook id=\"{}\" title=\"{}\" calculation=\"{}\">\n",
        escape_attr(&wb.id),
        escape_attr(&wb.title),
        escape_attr(&wb.calc_mode)
    );
    out.extend_from_slice(wb_tag.as_bytes());

    // 2. Dynamic Table of Contents (TOC)
    let toc = wb.compute_toc();
    let toc_tag = format!(
        "  <toc ref=\"{}\" extentX=\"{}\" extentY=\"{}\">\n",
        toc.pos.to_a1(),
        toc.extent_col,
        toc.extent_row
    );
    out.extend_from_slice(toc_tag.as_bytes());

    for entry in &toc.entries {
        let entry_line = format!(
            "    <entry sheet=\"{}\" ref=\"{}\" name=\"{}\" type=\"{}\" bytes=\"{}\" />\n",
            escape_attr(&entry.sheet_id),
            entry.coord.to_a1(),
            escape_attr(&entry.name),
            entry.type_name,
            entry.byte_size
        );
        out.extend_from_slice(entry_line.as_bytes());
    }
    out.extend_from_slice(b"  </toc>\n\n");

    // 3. Sheets and Cells
    for sheet in &wb.sheets {
        let sheet_header = format!(
            "  <sheet id=\"{}\" name=\"{}\">\n",
            escape_attr(&sheet.id),
            escape_attr(&sheet.name)
        );
        out.extend_from_slice(sheet_header.as_bytes());

        for cell in sheet.cells.values() {
            write_cell(&mut out, cell);
        }

        out.extend_from_slice(b"  </sheet>\n");
    }

    // 4. Embedded Documents
    if !wb.documents.is_empty() {
        out.extend_from_slice(b"\n  <documents>\n");
        for doc in &wb.documents {
            let doc_header = format!(
                "    <document id=\"{}\" title=\"{}\" type=\"{}\" bytes=\"{}\">",
                escape_attr(&doc.id),
                escape_attr(&doc.title),
                escape_attr(&doc.content_type),
                doc.body.as_bytes().len()
            );
            out.extend_from_slice(doc_header.as_bytes());
            out.extend_from_slice(doc.body.as_bytes());
            out.extend_from_slice(b"</document>\n");
        }
        out.extend_from_slice(b"  </documents>\n");
    }

    // 5. Historical Z-layer overlays. Base sheet content is Z=0.
    if !wb.history.is_empty() {
        out.extend_from_slice(b"\n  <history>\n");
        for (sheet_id, history) in &wb.history {
            for (z, records) in &history.layers {
                out.extend_from_slice(
                    format!("    <layer sheet=\"{}\" z=\"{}\">\n", escape_attr(sheet_id), z).as_bytes(),
                );
                for record in records {
                    if let Some(cell) = &record.cell {
                        write_cell(&mut out, cell);
                    } else {
                        out.extend_from_slice(
                            format!("    <delete ref=\"{}\" />\n", record.coord.to_a1()).as_bytes(),
                        );
                    }
                }
                out.extend_from_slice(b"    </layer>\n");
            }
        }
        out.extend_from_slice(b"  </history>\n");
    }

    // 6. Closing Tag
    out.extend_from_slice(b"</workbook>\n");

    out
}

fn write_cell(out: &mut Vec<u8>, cell: &Cell) {
    let mut attrs = format!(" ref=\"{}\"", cell.coord.to_a1());

    if let Some(ref id) = cell.id {
        attrs.push_str(&format!(" id=\"{}\"", escape_attr(id)));
    }

    if let Some(ref name) = cell.name {
        attrs.push_str(&format!(" name=\"{}\"", escape_attr(name)));
    }
    if !cell.document_refs.is_empty() {
        attrs.push_str(&format!(
            " documentRefs=\"{}\"",
            escape_attr(&cell.document_refs.join(","))
        ));
    }

    match &cell.value {
        CellValue::Empty => {
            let line = format!("    <cell{} type=\"empty\" />\n", attrs);
            out.extend_from_slice(line.as_bytes());
        }
        CellValue::Number(n) => {
            let line = format!("    <cell{} type=\"number\">{}</cell>\n", attrs, n);
            out.extend_from_slice(line.as_bytes());
        }
        CellValue::Text(t) => {
            let line = format!(
                "    <cell{} type=\"text\">{}</cell>\n",
                attrs,
                escape_xml_text(t)
            );
            out.extend_from_slice(line.as_bytes());
        }
        CellValue::Boolean(b) => {
            let line = format!("    <cell{} type=\"boolean\">{}</cell>\n", attrs, b);
            out.extend_from_slice(line.as_bytes());
        }
        CellValue::Formula {
            expr,
            cached_num,
            cached_text,
            cached_bool,
        } => {
            let mut val_str = String::new();
            let mut cached_type = String::new();
            if let Some(n) = cached_num {
                val_str = format!("{}", n);
                cached_type = "number".into();
            } else if let Some(value) = cached_bool {
                val_str = value.to_string();
                cached_type = "boolean".into();
            } else if let Some(ref t) = cached_text {
                val_str = escape_xml_text(t);
                cached_type = "text".into();
            }

            let cache_attr = if cached_type.is_empty() { String::new() } else {
                format!(" cachedType=\"{}\"", cached_type)
            };

            let line = format!(
                "    <cell{} type=\"formula\" formula=\"{}\"{}>{}</cell>\n",
                attrs,
                escape_attr(expr),
                cache_attr,
                val_str
            );
            out.extend_from_slice(line.as_bytes());
        }
        CellValue::Basic { source } => {
            if !cell.basic_reads.is_empty() {
                attrs.push_str(&format!(" reads=\"{}\"", escape_attr(&cell.basic_reads.join(","))));
            }
            if !cell.basic_writes.is_empty() {
                attrs.push_str(&format!(" writes=\"{}\"", escape_attr(&cell.basic_writes.join(","))));
            }
            let open = format!("    <cell{} type=\"basic\" bytes=\"{}\">", attrs, source.as_bytes().len());
            out.extend_from_slice(open.as_bytes());
            out.extend_from_slice(source.as_bytes());
            out.extend_from_slice(b"</cell>\n");
        }
        CellValue::Blob { bytes } => {
            let open = format!(
                "    <cell{} type=\"blob\" bytes=\"{}\">",
                attrs,
                bytes.len()
            );
            out.extend_from_slice(open.as_bytes());
            // Embed exact, uncompressed raw payload bytes
            out.extend_from_slice(bytes);
            out.extend_from_slice(b"</cell>\n");
        }
        CellValue::Error(e) => {
            let line = format!(
                "    <cell{} type=\"error\">{}</cell>\n",
                attrs,
                escape_xml_text(e)
            );
            out.extend_from_slice(line.as_bytes());
        }
    }
}

fn escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("&quot;"),
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

fn escape_xml_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}
