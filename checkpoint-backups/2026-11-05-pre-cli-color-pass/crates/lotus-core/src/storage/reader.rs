//! Deserialization of Lotus 5-7-11 workbooks from the uncompressed hybrid format.

#[cfg(not(feature = "std"))]
use alloc::{string::String, string::ToString, vec::Vec};

use crate::model::{
    cell::{Cell, CellValue},
    coord::Coord,
    sheet::Sheet,
    workbook::Workbook,
};

#[derive(Debug)]
pub enum ParseError {
    UnexpectedEof,
    InvalidTag(usize),
    MalformedAttribute(usize),
    MissingAttribute(&'static str),
    InvalidHistory(String),
    InvalidCoordinate(String),
    InvalidNumber(String),
    InvalidFormulaCache(String),
    BlobOverflow { expected: usize, available: usize },
}

pub fn deserialize_workbook(input: &[u8]) -> Result<Workbook, ParseError> {
    let mut scanner = Scanner::new(input);
    let mut workbook = Workbook::new("wb", "Lotus Workbook");
    let mut current_sheet: Option<Sheet> = None;
    let mut current_layer: Option<(String, u32, Vec<(Coord, Option<Cell>)>)> = None;

    while !scanner.is_eof() {
        scanner.skip_whitespace_and_comments();
        if scanner.is_eof() {
            break;
        }

        if scanner.peek() == Some(b'<') {
            let tag = scanner.parse_tag_header()?;

            match tag.name {
                "workbook" => {
                    if !tag.is_closing {
                        if let Some(id) = tag.get_attr_unescaped("id") {
                            workbook.id = id;
                        }
                        if let Some(title) = tag.get_attr_unescaped("title") {
                            workbook.title = title;
                        }
                        if let Some(calc) = tag.get_attr("calculation") {
                            workbook.calc_mode = calc.to_string();
                        }
                    }
                }
                "toc" => {
                    // Skip over TOC contents during read, as TOC is calculated dynamically on demand
                    if !tag.is_self_closing && !tag.is_closing {
                        scanner.skip_until_closing_tag("toc")?;
                    }
                }
                "sheet" => {
                    if tag.is_closing {
                        if let Some(sheet) = current_sheet.take() {
                            workbook.sheets.push(sheet);
                        }
                    } else {
                        let id = tag.get_attr_unescaped("id").unwrap_or_else(|| "sheet".to_string());
                        let name = tag.get_attr_unescaped("name").unwrap_or_else(|| "Sheet".to_string());
                        current_sheet = Some(Sheet::new(id, name));
                    }
                }
                "documents" => {
                    // Container tag, scanner will process child document tags
                }
                "history" => {
                    // Container tag, child layers are parsed below.
                }
                "layer" => {
                    if tag.is_closing {
                        let (sheet_id, z, changes) = current_layer.take().ok_or_else(|| {
                            ParseError::InvalidHistory("Closing layer without an open layer".into())
                        })?;
                        if workbook.get_sheet(&sheet_id).is_none() {
                            return Err(ParseError::InvalidHistory(format!("Unknown history sheet '{}'", sheet_id)));
                        }
                        workbook.history.entry(sheet_id.clone()).or_insert_with(crate::history::HistoryEngine::new)
                            .insert_layer(z, changes)
                            .map_err(|message| ParseError::InvalidHistory(message.into()))?;
                    } else {
                        if current_layer.is_some() {
                            return Err(ParseError::InvalidHistory("Nested history layers are not allowed".into()));
                        }
                        let sheet_id = tag.get_attr_unescaped("sheet")
                            .ok_or(ParseError::MissingAttribute("sheet"))?;
                        let z = tag.get_attr("z").and_then(|value| value.parse::<u32>().ok())
                            .ok_or(ParseError::MissingAttribute("z"))?;
                        if tag.is_self_closing {
                            if workbook.get_sheet(&sheet_id).is_none() {
                                return Err(ParseError::InvalidHistory(format!("Unknown history sheet '{}'", sheet_id)));
                            }
                            workbook.history.entry(sheet_id.clone()).or_insert_with(crate::history::HistoryEngine::new)
                                .insert_layer(z, Vec::new())
                                .map_err(|message| ParseError::InvalidHistory(message.into()))?;
                        } else {
                            current_layer = Some((sheet_id, z, Vec::new()));
                        }
                    }
                }
                "delete" => {
                    if tag.is_closing || !tag.is_self_closing {
                        return Err(ParseError::InvalidHistory("History deletions must use self-closing <delete /> tags".into()));
                    }
                    let (_, _, changes) = current_layer.as_mut().ok_or_else(|| {
                        ParseError::InvalidHistory("Deletion outside a history layer".into())
                    })?;
                    let coord_str = tag.get_attr("ref").ok_or(ParseError::MissingAttribute("ref"))?;
                    let coord = Coord::parse_a1(coord_str)
                        .ok_or_else(|| ParseError::InvalidCoordinate(coord_str.to_string()))?;
                    changes.push((coord, None));
                }
                "document" => {
                    if !tag.is_closing {
                        let id = tag.get_attr_unescaped("id").unwrap_or_else(|| "doc".to_string());
                        let title = tag.get_attr_unescaped("title").unwrap_or_else(|| "Document".to_string());
                        let doc_type = tag.get_attr_unescaped("type").unwrap_or_else(|| "text/markdown".to_string());
                        let body = if let Some(len) = tag.get_attr("bytes").and_then(|value| value.parse::<usize>().ok()) {
                            let bytes = scanner.consume_exact_bytes(len)?;
                            scanner.expect_closing_tag("document")?;
                            core::str::from_utf8(bytes).unwrap_or("").to_string()
                        } else {
                            let inner_bytes = scanner.consume_until_closing_tag("document")?;
                            let mut s = core::str::from_utf8(inner_bytes).unwrap_or("").trim();
                            if s.starts_with("<![CDATA[") && s.ends_with("]]>") {
                                s = &s[9..s.len() - 3];
                                s = s.trim();
                            }
                            s.to_string()
                        };
                        let mut doc = crate::doc::Document::new(id, title, body);
                        doc.content_type = doc_type.to_string();
                        workbook.documents.push(doc);
                    }
                }
                "cell" => {
                    if tag.is_closing {
                        // Handled inline below
                    } else {
                        let coord_str = tag
                            .get_attr("ref")
                            .ok_or(ParseError::MissingAttribute("ref"))?;
                        let coord = Coord::parse_a1(coord_str)
                            .ok_or_else(|| ParseError::InvalidCoordinate(coord_str.to_string()))?;

                        let cell_type = tag.get_attr("type").unwrap_or("empty");
                        let cell_id = tag.get_attr_unescaped("id");
                        let cell_name = tag.get_attr_unescaped("name");

                        let value = if tag.is_self_closing {
                            CellValue::Empty
                        } else {
                            match cell_type {
                                "blob" => {
                                    let bytes_len: usize = tag
                                        .get_attr("bytes")
                                        .and_then(|s| s.parse().ok())
                                        .unwrap_or(0);

                                    let raw_slice = scanner.consume_exact_bytes(bytes_len)?;
                                    scanner.expect_closing_tag("cell")?;
                                    CellValue::Blob {
                                        bytes: raw_slice.to_vec(),
                                    }
                                }
                                "basic" => {
                                    let source = if let Some(len) = tag.get_attr("bytes").and_then(|value| value.parse::<usize>().ok()) {
                                        let bytes = scanner.consume_exact_bytes(len)?;
                                        scanner.expect_closing_tag("cell")?;
                                        core::str::from_utf8(bytes).unwrap_or("").to_string()
                                    } else {
                                        let inner_bytes = scanner.consume_until_closing_tag("cell")?;
                                        let mut s = core::str::from_utf8(inner_bytes).unwrap_or("").trim();
                                        if s.starts_with("<![CDATA[") && s.ends_with("]]>") {
                                            s = &s[9..s.len() - 3];
                                            s = s.trim();
                                        }
                                        s.to_string()
                                    };
                                    CellValue::Basic { source }
                                }
                                "formula" => {
                                    let expr = tag.get_attr_unescaped("formula").unwrap_or_default();
                                    let inner_bytes = scanner.consume_until_closing_tag("cell")?;
                                    let inner_str = core::str::from_utf8(inner_bytes).unwrap_or("");
                                    let cached_type = tag.get_attr("cachedType");
                                    let (cached_num, cached_text, cached_bool) = match cached_type {
                                        Some("number") => {
                                            let value = inner_str.trim().parse::<f64>()
                                                .map_err(|_| ParseError::InvalidNumber(inner_str.trim().to_string()))?;
                                            (Some(value), None, None)
                                        }
                                        Some("boolean") => (None, None, Some(inner_str.trim().eq_ignore_ascii_case("true"))),
                                        Some("text") => (None, Some(unescape_xml(inner_str)), None),
                                        Some("empty") | None => {
                                            let legacy = inner_str.trim();
                                            let number = legacy.parse::<f64>().ok();
                                            let text = if number.is_none() && !legacy.is_empty() { Some(unescape_xml(legacy)) } else { None };
                                            (number, text, None)
                                        }
                                        Some(other) => return Err(ParseError::InvalidFormulaCache(other.to_string())),
                                    };

                                    CellValue::Formula {
                                        expr,
                                        cached_num,
                                        cached_text,
                                        cached_bool,
                                    }
                                }
                                "number" => {
                                    let inner_bytes = scanner.consume_until_closing_tag("cell")?;
                                    let s = core::str::from_utf8(inner_bytes)
                                        .unwrap_or("")
                                        .trim();
                                    let n = s
                                        .parse::<f64>()
                                        .map_err(|_| ParseError::InvalidNumber(s.to_string()))?;
                                    CellValue::Number(n)
                                }
                                "text" => {
                                    let inner_bytes = scanner.consume_until_closing_tag("cell")?;
                                    let s = core::str::from_utf8(inner_bytes).unwrap_or("");
                                    CellValue::Text(unescape_xml(s))
                                }
                                "boolean" => {
                                    let inner_bytes = scanner.consume_until_closing_tag("cell")?;
                                    let s = core::str::from_utf8(inner_bytes)
                                        .unwrap_or("")
                                        .trim();
                                    CellValue::Boolean(s.eq_ignore_ascii_case("true"))
                                }
                                "error" => {
                                    let inner_bytes = scanner.consume_until_closing_tag("cell")?;
                                    let s = core::str::from_utf8(inner_bytes)
                                        .unwrap_or("")
                                        .trim();
                                    CellValue::Error(unescape_xml(s))
                                }
                                _ => {
                                    scanner.consume_until_closing_tag("cell")?;
                                    CellValue::Empty
                                }
                            }
                        };

                        let mut cell = Cell::new(coord, value);
                        cell.id = cell_id;
                        cell.name = cell_name;
                        if matches!(&cell.value, CellValue::Basic { .. }) {
                            cell.basic_reads = tag.get_attr_unescaped("reads").unwrap_or_default()
                                .split(',').map(str::trim).filter(|name| !name.is_empty()).map(ToString::to_string).collect();
                            cell.basic_writes = tag.get_attr_unescaped("writes").unwrap_or_default()
                                .split(',').map(str::trim).filter(|name| !name.is_empty()).map(ToString::to_string).collect();
                        }

                        if let Some((_, _, changes)) = current_layer.as_mut() {
                            changes.push((coord, Some(cell)));
                        } else if let Some(ref mut sheet) = current_sheet {
                            sheet.set_cell(cell);
                        }
                    }
                }
                _ => {
                    // Ignore unknown tags safely
                    if !tag.is_self_closing && !tag.is_closing {
                        let _ = scanner.skip_until_closing_tag(tag.name);
                    }
                }
            }
        } else {
            scanner.advance(1);
        }
    }

    if current_layer.is_some() {
        return Err(ParseError::UnexpectedEof);
    }
    if let Some(sheet) = current_sheet.take() {
        workbook.sheets.push(sheet);
    }

    Ok(workbook)
}

struct Tag<'a> {
    name: &'a str,
    is_closing: bool,
    is_self_closing: bool,
    attrs: Vec<(&'a str, &'a str)>,
}

impl<'a> Tag<'a> {
    /// Attribute value with XML entities decoded (for free-text attributes: id, name, title, formula).
    fn get_attr_unescaped(&self, key: &str) -> Option<String> {
        self.get_attr(key).map(unescape_xml)
    }

    fn get_attr(&self, key: &str) -> Option<&'a str> {
        self.attrs
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| *v)
    }
}

struct Scanner<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Scanner<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.data.len()
    }

    fn peek(&self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }

    fn advance(&mut self, n: usize) {
        self.pos = (self.pos + n).min(self.data.len());
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            // Whitespace
            while self.pos < self.data.len() && self.data[self.pos].is_ascii_whitespace() {
                self.pos += 1;
            }

            // Comments
            if self.pos + 4 <= self.data.len() && &self.data[self.pos..self.pos + 4] == b"<!--" {
                self.pos += 4;
                while self.pos + 3 <= self.data.len() {
                    if &self.data[self.pos..self.pos + 3] == b"-->" {
                        self.pos += 3;
                        break;
                    }
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    fn parse_tag_header(&mut self) -> Result<Tag<'a>, ParseError> {
        if self.peek() != Some(b'<') {
            return Err(ParseError::InvalidTag(self.pos));
        }
        self.advance(1); // skip '<'

        let is_closing = if self.peek() == Some(b'/') {
            self.advance(1);
            true
        } else {
            false
        };

        // Extract tag name
        let name_start = self.pos;
        while self.pos < self.data.len()
            && !self.data[self.pos].is_ascii_whitespace()
            && self.data[self.pos] != b'>'
            && self.data[self.pos] != b'/'
        {
            self.pos += 1;
        }

        let name = core::str::from_utf8(&self.data[name_start..self.pos])
            .map_err(|_| ParseError::InvalidTag(name_start))?;

        let mut attrs = Vec::new();
        let mut is_self_closing = false;

        // Parse attributes
        loop {
            while self.pos < self.data.len() && self.data[self.pos].is_ascii_whitespace() {
                self.pos += 1;
            }

            if self.pos >= self.data.len() {
                return Err(ParseError::UnexpectedEof);
            }

            if self.data[self.pos] == b'/' {
                is_self_closing = true;
                self.advance(1);
                continue;
            }

            if self.data[self.pos] == b'>' {
                self.advance(1); // end of tag
                break;
            }

            // Read attribute key
            let key_start = self.pos;
            while self.pos < self.data.len()
                && self.data[self.pos] != b'='
                && !self.data[self.pos].is_ascii_whitespace()
                && self.data[self.pos] != b'>'
                && self.data[self.pos] != b'/'
            {
                self.pos += 1;
            }

            let key = core::str::from_utf8(&self.data[key_start..self.pos])
                .map_err(|_| ParseError::MalformedAttribute(key_start))?;

            while self.pos < self.data.len() && self.data[self.pos].is_ascii_whitespace() {
                self.pos += 1;
            }

            if self.pos < self.data.len() && self.data[self.pos] == b'=' {
                self.advance(1);
                while self.pos < self.data.len() && self.data[self.pos].is_ascii_whitespace() {
                    self.pos += 1;
                }

                let quote = self.peek().ok_or(ParseError::UnexpectedEof)?;
                if quote == b'"' || quote == b'\'' {
                    self.advance(1);
                    let val_start = self.pos;
                    while self.pos < self.data.len() && self.data[self.pos] != quote {
                        self.pos += 1;
                    }
                    let val = core::str::from_utf8(&self.data[val_start..self.pos])
                        .map_err(|_| ParseError::MalformedAttribute(val_start))?;
                    self.advance(1); // consume closing quote
                    attrs.push((key, val));
                }
            }
        }

        Ok(Tag {
            name,
            is_closing,
            is_self_closing,
            attrs,
        })
    }

    fn consume_exact_bytes(&mut self, n: usize) -> Result<&'a [u8], ParseError> {
        if self.pos + n > self.data.len() {
            return Err(ParseError::BlobOverflow {
                expected: n,
                available: self.data.len().saturating_sub(self.pos),
            });
        }
        let slice = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    fn consume_until_closing_tag(&mut self, tag: &str) -> Result<&'a [u8], ParseError> {
        let needle = [b'<', b'/'];
        let start = self.pos;

        while self.pos + 2 + tag.len() + 1 <= self.data.len() {
            if &self.data[self.pos..self.pos + 2] == &needle {
                let tag_slice = &self.data[self.pos + 2..self.pos + 2 + tag.len()];
                if tag_slice.eq_ignore_ascii_case(tag.as_bytes()) {
                    let content = &self.data[start..self.pos];
                    self.pos += 2 + tag.len();
                    while self.pos < self.data.len() && self.data[self.pos] != b'>' {
                        self.pos += 1;
                    }
                    if self.pos < self.data.len() {
                        self.pos += 1; // skip '>'
                    }
                    return Ok(content);
                }
            }
            self.pos += 1;
        }

        Err(ParseError::UnexpectedEof)
    }

    fn expect_closing_tag(&mut self, tag: &str) -> Result<(), ParseError> {
        self.skip_whitespace_and_comments();
        if self.pos + 2 + tag.len() + 1 > self.data.len() {
            return Err(ParseError::UnexpectedEof);
        }
        if &self.data[self.pos..self.pos + 2] != b"</" {
            return Err(ParseError::InvalidTag(self.pos));
        }
        self.pos += 2;
        let tag_slice = &self.data[self.pos..self.pos + tag.len()];
        if !tag_slice.eq_ignore_ascii_case(tag.as_bytes()) {
            return Err(ParseError::InvalidTag(self.pos));
        }
        self.pos += tag.len();
        while self.pos < self.data.len() && self.data[self.pos] != b'>' {
            self.pos += 1;
        }
        if self.pos < self.data.len() {
            self.pos += 1;
        }
        Ok(())
    }

    fn skip_until_closing_tag(&mut self, tag: &str) -> Result<(), ParseError> {
        let _ = self.consume_until_closing_tag(tag)?;
        Ok(())
    }
}

fn unescape_xml(s: &str) -> String {
    // `&amp;` must be decoded last, otherwise "&amp;lt;" would collapse to "<".
    s.replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}
