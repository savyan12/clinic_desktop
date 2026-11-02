use super::error::{ApiError, ApiResult};
use printpdf::{
    Color as PdfColor, FontId, Mm, Op, ParsedFont, PdfDocument, PdfPage, PdfSaveOptions, Pt, Rect,
    Rgb, TextItem, TextMatrix,
};
use rust_xlsxwriter::{Color as XlsxColor, Format, FormatAlign, FormatBorder, Workbook, XlsxError};
use serde_json::Value;
use std::collections::HashMap;
use unicode_bidi::{BidiInfo, Level};

const REGULAR_FONT: &[u8] = include_bytes!("../../assets/fonts/DejaVuSans.ttf");
const BOLD_FONT: &[u8] = include_bytes!("../../assets/fonts/DejaVuSans-Bold.ttf");

pub struct ExportedReport {
    pub bytes: Vec<u8>,
    pub filename: String,
    pub content_type: &'static str,
}

#[derive(Clone, Copy)]
struct ExportColumn {
    key: &'static str,
    label: &'static str,
    xlsx_width: f64,
    pdf_weight: f32,
}

pub fn export(report_type: &str, format: &str, report: &Value) -> ApiResult<ExportedReport> {
    match format {
        "xlsx" => export_xlsx(report_type, report),
        "pdf" => export_pdf(report_type, report),
        _ => {
            Err(ApiError::validation("صيغة التصدير غير مدعومة.")
                .field("format", "اختر xlsx أو pdf."))
        }
    }
}

fn export_xlsx(report_type: &str, report: &Value) -> ApiResult<ExportedReport> {
    let columns = columns(report_type)?;
    let rows = report
        .get("rows")
        .and_then(Value::as_array)
        .ok_or_else(|| ApiError::internal("Report rows are missing."))?;
    let totals = report
        .get("totals")
        .and_then(Value::as_object)
        .ok_or_else(|| ApiError::internal("Report totals are missing."))?;
    let mut workbook = Workbook::new();
    {
        let worksheet = workbook.add_worksheet();
        worksheet.set_right_to_left(true);
        worksheet.set_name("التقرير").map_err(xlsx_error)?;
        worksheet.set_freeze_panes(4, 0).map_err(xlsx_error)?;

        let title_format = Format::new()
            .set_bold()
            .set_font_size(18)
            .set_font_color(XlsxColor::White)
            .set_background_color("#0066CC")
            .set_align(FormatAlign::Center)
            .set_align(FormatAlign::VerticalCenter)
            .set_reading_direction(2);
        let meta_format = Format::new()
            .set_font_color("#5F6368")
            .set_align(FormatAlign::Right)
            .set_reading_direction(2);
        let header_format = Format::new()
            .set_bold()
            .set_background_color("#F5F5F7")
            .set_border(FormatBorder::Thin)
            .set_border_color("#E0E0E0")
            .set_align(FormatAlign::Right)
            .set_align(FormatAlign::VerticalCenter)
            .set_text_wrap()
            .set_reading_direction(2);
        let cell_format = Format::new()
            .set_border(FormatBorder::Thin)
            .set_border_color("#E0E0E0")
            .set_align(FormatAlign::Right)
            .set_align(FormatAlign::VerticalCenter)
            .set_text_wrap()
            .set_reading_direction(2);
        let total_label_format = Format::new()
            .set_bold()
            .set_background_color("#F5F5F7")
            .set_border(FormatBorder::Thin)
            .set_border_color("#E0E0E0")
            .set_align(FormatAlign::Right)
            .set_reading_direction(2);
        let total_value_format = Format::new()
            .set_border(FormatBorder::Thin)
            .set_border_color("#E0E0E0")
            .set_align(FormatAlign::Right)
            .set_reading_direction(2);

        let last_column = u16::try_from(columns.len().saturating_sub(1))
            .map_err(|error| ApiError::internal(format!("Invalid report column count: {error}")))?;
        worksheet
            .merge_range(
                0,
                0,
                0,
                last_column,
                report_title(report_type),
                &title_format,
            )
            .map_err(xlsx_error)?;
        worksheet.set_row_height(0, 30).map_err(xlsx_error)?;
        worksheet
            .merge_range(1, 0, 1, last_column, &period_label(report), &meta_format)
            .map_err(xlsx_error)?;
        worksheet
            .merge_range(2, 0, 2, last_column, &generated_label(report), &meta_format)
            .map_err(xlsx_error)?;

        for (column_index, column) in columns.iter().enumerate() {
            let column_index = u16::try_from(column_index)
                .map_err(|error| ApiError::internal(format!("Invalid column index: {error}")))?;
            worksheet
                .set_column_width(column_index, column.xlsx_width)
                .map_err(xlsx_error)?;
            worksheet
                .write_string_with_format(3, column_index, column.label, &header_format)
                .map_err(xlsx_error)?;
        }
        worksheet.set_row_height(3, 25).map_err(xlsx_error)?;

        for (row_index, row) in rows.iter().enumerate() {
            let row_index = u32::try_from(row_index + 4)
                .map_err(|error| ApiError::internal(format!("Invalid row index: {error}")))?;
            for (column_index, column) in columns.iter().enumerate() {
                let column_index = u16::try_from(column_index).map_err(|error| {
                    ApiError::internal(format!("Invalid column index: {error}"))
                })?;
                let value = row
                    .get(column.key)
                    .map(|value| display_value(column.key, value))
                    .unwrap_or_default();
                worksheet
                    .write_string_with_format(row_index, column_index, &value, &cell_format)
                    .map_err(xlsx_error)?;
            }
            worksheet
                .set_row_height(row_index, 22)
                .map_err(xlsx_error)?;
        }

        let totals_start = u32::try_from(rows.len() + 5)
            .map_err(|error| ApiError::internal(format!("Invalid totals row: {error}")))?;
        worksheet
            .write_string_with_format(totals_start, 0, "الإجماليات", &total_label_format)
            .map_err(xlsx_error)?;
        for (offset, (key, value)) in totals.iter().enumerate() {
            let row = totals_start
                + u32::try_from(offset + 1)
                    .map_err(|error| ApiError::internal(format!("Invalid totals row: {error}")))?;
            worksheet
                .write_string_with_format(row, 0, heading_label(key), &total_label_format)
                .map_err(xlsx_error)?;
            worksheet
                .write_string_with_format(row, 1, &display_value(key, value), &total_value_format)
                .map_err(xlsx_error)?;
        }
    }

    let bytes = workbook.save_to_buffer().map_err(xlsx_error)?;
    Ok(ExportedReport {
        bytes,
        filename: format!("تقرير-{report_type}.xlsx"),
        content_type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    })
}

fn export_pdf(report_type: &str, report: &Value) -> ApiResult<ExportedReport> {
    let columns = columns(report_type)?;
    let rows = report
        .get("rows")
        .and_then(Value::as_array)
        .ok_or_else(|| ApiError::internal("Report rows are missing."))?;
    let totals = report
        .get("totals")
        .and_then(Value::as_object)
        .ok_or_else(|| ApiError::internal("Report totals are missing."))?;

    let mut warnings = Vec::new();
    let regular = ParsedFont::from_bytes(REGULAR_FONT, 0, &mut warnings)
        .ok_or_else(|| ApiError::internal("Unable to load the Arabic PDF font."))?;
    let bold = ParsedFont::from_bytes(BOLD_FONT, 0, &mut warnings)
        .ok_or_else(|| ApiError::internal("Unable to load the Arabic PDF bold font."))?;
    let regular_face = rustybuzz::Face::from_slice(REGULAR_FONT, 0)
        .ok_or_else(|| ApiError::internal("Unable to shape the Arabic PDF font."))?;
    let bold_face = rustybuzz::Face::from_slice(BOLD_FONT, 0)
        .ok_or_else(|| ApiError::internal("Unable to shape the Arabic PDF bold font."))?;

    let mut document = PdfDocument::new(report_title(report_type));
    let regular_id = document.add_font(&regular);
    let bold_id = document.add_font(&bold);
    let fonts = PdfFonts {
        regular_id,
        regular_face,
        regular_glyph_chars: glyph_char_map(REGULAR_FONT)?,
        bold_id,
        bold_face,
        bold_glyph_chars: glyph_char_map(BOLD_FONT)?,
    };

    let page_width = Mm(297.0).into_pt();
    let page_height = Mm(210.0).into_pt();
    let margin = Pt(32.0);
    let table_width = page_width - margin - margin;
    let row_height = Pt(24.0);
    let header_height = Pt(27.0);
    let rows_per_page = 14usize;
    let page_count = rows.len().max(1).div_ceil(rows_per_page);
    let widths = pdf_column_widths(&columns, table_width);
    let mut pages = Vec::with_capacity(page_count);

    for page_index in 0..page_count {
        let start = page_index * rows_per_page;
        let end = (start + rows_per_page).min(rows.len());
        let mut ops = Vec::new();
        draw_page_header(
            &mut ops,
            &fonts,
            report_type,
            report,
            page_width,
            page_height,
            page_index + 1,
            page_count,
        );
        let table_top = page_height - Pt(92.0);
        draw_table_row(
            &mut ops,
            &fonts,
            &columns,
            &widths,
            margin,
            table_top,
            header_height,
            None,
            true,
        );

        if rows.is_empty() {
            draw_box(
                &mut ops,
                Rect {
                    x: margin,
                    y: table_top - header_height,
                    width: table_width,
                    height: row_height,
                },
                PdfColor::Rgb(rgb(1.0, 1.0, 1.0)),
                true,
            );
            draw_text_right(
                &mut ops,
                &fonts.regular_face,
                &fonts.regular_glyph_chars,
                &fonts.regular_id,
                "لا توجد بيانات ضمن الفترة المحددة.",
                Pt(10.0),
                page_width - margin - Pt(8.0),
                table_top - header_height - Pt(16.0),
                table_width - Pt(16.0),
                ink(),
            );
        } else {
            for (offset, row) in rows[start..end].iter().enumerate() {
                let top = table_top
                    - header_height
                    - Pt(row_height.0 * u32::try_from(offset).unwrap_or(0) as f32);
                draw_table_row(
                    &mut ops,
                    &fonts,
                    &columns,
                    &widths,
                    margin,
                    top,
                    row_height,
                    Some(row),
                    false,
                );
            }
        }

        if page_index + 1 == page_count {
            let used_rows = end.saturating_sub(start).max(1);
            let totals_top =
                table_top - header_height - Pt(row_height.0 * used_rows as f32) - Pt(18.0);
            draw_totals(&mut ops, &fonts, totals, page_width - margin, totals_top);
        }
        pages.push(PdfPage::new(Mm(297.0), Mm(210.0), ops));
    }

    document.with_pages(pages);
    let bytes = document.save(&PdfSaveOptions::default(), &mut warnings);
    if !bytes.starts_with(b"%PDF-") {
        return Err(ApiError::internal("The generated PDF is invalid."));
    }
    Ok(ExportedReport {
        bytes,
        filename: format!("تقرير-{report_type}.pdf"),
        content_type: "application/pdf",
    })
}

struct PdfFonts<'a> {
    regular_id: FontId,
    regular_face: rustybuzz::Face<'a>,
    regular_glyph_chars: HashMap<u16, char>,
    bold_id: FontId,
    bold_face: rustybuzz::Face<'a>,
    bold_glyph_chars: HashMap<u16, char>,
}

fn draw_page_header(
    ops: &mut Vec<Op>,
    fonts: &PdfFonts<'_>,
    report_type: &str,
    report: &Value,
    page_width: Pt,
    page_height: Pt,
    page_number: usize,
    page_count: usize,
) {
    draw_box(
        ops,
        Rect {
            x: Pt(0.0),
            y: page_height,
            width: page_width,
            height: Pt(58.0),
        },
        PdfColor::Rgb(rgb(0.0, 0.4, 0.8)),
        false,
    );
    draw_text_right(
        ops,
        &fonts.bold_face,
        &fonts.bold_glyph_chars,
        &fonts.bold_id,
        report_title(report_type),
        Pt(19.0),
        page_width - Pt(32.0),
        page_height - Pt(31.0),
        page_width - Pt(64.0),
        PdfColor::Rgb(rgb(1.0, 1.0, 1.0)),
    );
    let meta = format!(
        "{} | {} | صفحة {} من {}",
        period_label(report),
        generated_label(report),
        page_number,
        page_count
    );
    draw_text_right(
        ops,
        &fonts.regular_face,
        &fonts.regular_glyph_chars,
        &fonts.regular_id,
        &meta,
        Pt(8.5),
        page_width - Pt(32.0),
        page_height - Pt(48.0),
        page_width - Pt(64.0),
        PdfColor::Rgb(rgb(0.9, 0.94, 1.0)),
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_table_row(
    ops: &mut Vec<Op>,
    fonts: &PdfFonts<'_>,
    columns: &[ExportColumn],
    widths: &[Pt],
    left: Pt,
    top: Pt,
    height: Pt,
    row: Option<&Value>,
    header: bool,
) {
    let mut right = left
        + widths
            .iter()
            .copied()
            .fold(Pt(0.0), |sum, width| sum + width);
    for (index, column) in columns.iter().enumerate() {
        let width = widths[index];
        let rect = Rect {
            x: right - width,
            y: top,
            width,
            height,
        };
        draw_box(
            ops,
            rect.clone(),
            if header {
                PdfColor::Rgb(rgb(0.961, 0.961, 0.969))
            } else {
                PdfColor::Rgb(rgb(1.0, 1.0, 1.0))
            },
            true,
        );
        let text = if header {
            column.label.to_string()
        } else {
            row.and_then(|value| value.get(column.key))
                .map(|value| display_value(column.key, value))
                .unwrap_or_default()
        };
        let (face, glyph_chars, font_id, size) = if header {
            (
                &fonts.bold_face,
                &fonts.bold_glyph_chars,
                &fonts.bold_id,
                Pt(8.3),
            )
        } else {
            (
                &fonts.regular_face,
                &fonts.regular_glyph_chars,
                &fonts.regular_id,
                Pt(7.8),
            )
        };
        draw_text_right(
            ops,
            face,
            glyph_chars,
            font_id,
            &fit_text(&text, width - Pt(10.0), size),
            size,
            rect.x + rect.width - Pt(5.0),
            rect.y - Pt((rect.height.0 + size.0) / 2.0) + Pt(2.0),
            rect.width - Pt(10.0),
            ink(),
        );
        right = rect.x;
    }
}

fn draw_totals(
    ops: &mut Vec<Op>,
    fonts: &PdfFonts<'_>,
    totals: &serde_json::Map<String, Value>,
    right: Pt,
    top: Pt,
) {
    let label_width = Pt(135.0);
    let value_width = Pt(105.0);
    let height = Pt(21.0);
    draw_text_right(
        ops,
        &fonts.bold_face,
        &fonts.bold_glyph_chars,
        &fonts.bold_id,
        "الإجماليات",
        Pt(11.0),
        right,
        top - Pt(12.0),
        label_width + value_width,
        ink(),
    );
    for (index, (key, value)) in totals.iter().enumerate() {
        let row_top = top - Pt(20.0 + height.0 * index as f32);
        let value_rect = Rect {
            x: right - value_width,
            y: row_top,
            width: value_width,
            height,
        };
        let label_rect = Rect {
            x: value_rect.x - label_width,
            y: row_top,
            width: label_width,
            height,
        };
        draw_box(
            ops,
            label_rect.clone(),
            PdfColor::Rgb(rgb(0.961, 0.961, 0.969)),
            true,
        );
        draw_box(
            ops,
            value_rect.clone(),
            PdfColor::Rgb(rgb(1.0, 1.0, 1.0)),
            true,
        );
        draw_text_right(
            ops,
            &fonts.bold_face,
            &fonts.bold_glyph_chars,
            &fonts.bold_id,
            heading_label(key),
            Pt(8.5),
            label_rect.x + label_rect.width - Pt(5.0),
            row_top - Pt(13.5),
            label_rect.width - Pt(10.0),
            ink(),
        );
        draw_text_right(
            ops,
            &fonts.regular_face,
            &fonts.regular_glyph_chars,
            &fonts.regular_id,
            &display_value(key, value),
            Pt(8.5),
            value_rect.x + value_rect.width - Pt(5.0),
            row_top - Pt(13.5),
            value_rect.width - Pt(10.0),
            ink(),
        );
    }
}

fn draw_box(ops: &mut Vec<Op>, rect: Rect, fill: PdfColor, border: bool) {
    ops.push(Op::SaveGraphicsState);
    ops.push(Op::SetFillColor { col: fill });
    ops.push(Op::DrawPolygon {
        polygon: rect.to_polygon(),
    });
    if border {
        ops.push(Op::SetOutlineColor {
            col: PdfColor::Rgb(rgb(0.878, 0.878, 0.878)),
        });
        ops.push(Op::SetOutlineThickness { pt: Pt(0.55) });
        ops.push(Op::DrawLine {
            line: rect.to_line(),
        });
    }
    ops.push(Op::RestoreGraphicsState);
}

#[allow(clippy::too_many_arguments)]
fn draw_text_right(
    ops: &mut Vec<Op>,
    face: &rustybuzz::Face<'_>,
    glyph_chars: &HashMap<u16, char>,
    font_id: &FontId,
    text: &str,
    size: Pt,
    right: Pt,
    baseline: Pt,
    max_width: Pt,
    color: PdfColor,
) {
    let runs = shape_visual_runs(face, glyph_chars, text, size);
    let width = runs.iter().map(|run| run.width).sum::<f32>();
    let start = right.0 - width.min(max_width.0);
    let mut run_left = start;
    ops.push(Op::SaveGraphicsState);
    ops.push(Op::StartTextSection);
    ops.push(Op::SetFillColor { col: color });
    ops.push(Op::SetFontSize {
        size,
        font: font_id.clone(),
    });
    for run in runs {
        for glyph in run.glyphs {
            ops.push(Op::SetTextMatrix {
                matrix: TextMatrix::Translate(Pt(run_left + glyph.x), Pt(baseline.0 + glyph.y)),
            });
            ops.push(Op::WriteText {
                font: font_id.clone(),
                items: vec![TextItem::Text(glyph.character.to_string())],
            });
        }
        run_left += run.width;
    }
    ops.push(Op::EndTextSection);
    ops.push(Op::RestoreGraphicsState);
}

struct ShapedRun {
    glyphs: Vec<ShapedGlyph>,
    width: f32,
}

struct ShapedGlyph {
    character: char,
    x: f32,
    y: f32,
}

fn shape_visual_runs(
    face: &rustybuzz::Face<'_>,
    glyph_chars: &HashMap<u16, char>,
    text: &str,
    size: Pt,
) -> Vec<ShapedRun> {
    if text.is_empty() {
        return Vec::new();
    }
    let bidi = BidiInfo::new(text, Some(Level::rtl()));
    let Some(paragraph) = bidi.paragraphs.first() else {
        return Vec::new();
    };
    let (levels, visual_runs) = bidi.visual_runs(paragraph, paragraph.range.clone());
    visual_runs
        .into_iter()
        .filter_map(|range| {
            let run_text = text.get(range.clone())?;
            let rtl = levels.get(range.start).is_some_and(Level::is_rtl);
            Some(shape_run(face, glyph_chars, run_text, size, rtl))
        })
        .collect()
}

fn shape_run(
    face: &rustybuzz::Face<'_>,
    glyph_chars: &HashMap<u16, char>,
    text: &str,
    size: Pt,
    rtl: bool,
) -> ShapedRun {
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.set_direction(if rtl {
        rustybuzz::Direction::RightToLeft
    } else {
        rustybuzz::Direction::LeftToRight
    });
    buffer.guess_segment_properties();
    let shaped = rustybuzz::shape(face, &[], buffer);
    let scale = size.0 / face.units_per_em() as f32;
    let mut pen_x = 0.0f32;
    let mut pending = Vec::new();
    let mut min_x = 0.0f32;
    let mut max_x = 0.0f32;
    for (info, position) in shaped
        .glyph_infos()
        .iter()
        .zip(shaped.glyph_positions().iter())
    {
        let x = (pen_x + position.x_offset as f32) * scale;
        let y = position.y_offset as f32 * scale;
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        let source_character = text
            .get(info.cluster as usize..)
            .and_then(|value| value.chars().next())
            .unwrap_or(' ');
        let glyph_id = info.glyph_id as u16;
        let character = glyph_chars
            .get(&glyph_id)
            .copied()
            .unwrap_or(source_character);
        pending.push((character, x, y));
        pen_x += position.x_advance as f32;
    }
    let final_pen = pen_x * scale;
    min_x = min_x.min(final_pen);
    max_x = max_x.max(final_pen);
    let width = (max_x - min_x).max(size.0 * 0.2);
    ShapedRun {
        glyphs: pending
            .into_iter()
            .map(|(character, x, y)| ShapedGlyph {
                character,
                x: x - min_x,
                y,
            })
            .collect(),
        width,
    }
}

fn glyph_char_map(font_bytes: &[u8]) -> ApiResult<HashMap<u16, char>> {
    let face = ttf_parser::Face::parse(font_bytes, 0)
        .map_err(|error| ApiError::internal(format!("Unable to parse the PDF font: {error}")))?;
    let cmap = face
        .tables()
        .cmap
        .ok_or_else(|| ApiError::internal("The PDF font has no Unicode character map."))?;
    let mut glyph_chars = HashMap::new();
    let mut priorities = HashMap::<u16, u8>::new();

    for subtable in cmap.subtables {
        if !subtable.is_unicode() {
            continue;
        }
        subtable.codepoints(|codepoint| {
            let Some(character) = char::from_u32(codepoint) else {
                return;
            };
            let Some(glyph) = subtable.glyph_index(codepoint) else {
                return;
            };
            let priority = glyph_character_priority(character);
            if priorities
                .get(&glyph.0)
                .is_none_or(|current| priority > *current)
            {
                priorities.insert(glyph.0, priority);
                glyph_chars.insert(glyph.0, character);
            }
        });
    }

    Ok(glyph_chars)
}

fn glyph_character_priority(character: char) -> u8 {
    match character as u32 {
        0xFB50..=0xFDFF | 0xFE70..=0xFEFF => 3,
        0x0600..=0x06FF | 0x0750..=0x077F | 0x08A0..=0x08FF => 2,
        _ => 1,
    }
}

fn fit_text(value: &str, width: Pt, size: Pt) -> String {
    let max_chars = ((width.0 / (size.0 * 0.58)).floor() as usize).max(3);
    let count = value.chars().count();
    if count <= max_chars {
        return value.to_string();
    }
    let mut shortened = value
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    shortened.push_str("...");
    shortened
}

fn pdf_column_widths(columns: &[ExportColumn], total: Pt) -> Vec<Pt> {
    let weights = columns.iter().map(|column| column.pdf_weight).sum::<f32>();
    columns
        .iter()
        .map(|column| Pt(total.0 * column.pdf_weight / weights))
        .collect()
}

fn columns(report_type: &str) -> ApiResult<Vec<ExportColumn>> {
    let columns = match report_type {
        "sessions" => vec![
            column("opened_at", "تاريخ الجلسة", 22.0, 1.35),
            column("patient", "المريض", 24.0, 1.35),
            column("doctor", "الطبيب", 21.0, 1.15),
            column("services", "الخدمات", 38.0, 2.35),
            column("net_total", "القيمة الصافية", 16.0, 0.95),
            column("status", "الحالة", 13.0, 0.75),
        ],
        "collections" => vec![
            column("created_at", "التاريخ", 22.0, 1.2),
            column("type", "النوع", 22.0, 1.25),
            column("patient", "المريض", 25.0, 1.45),
            column("method", "طريقة الدفع", 18.0, 1.0),
            column("amount", "المبلغ", 16.0, 0.9),
        ],
        "debts" => vec![
            column("id", "الرقم", 10.0, 0.55),
            column("patient", "المريض", 23.0, 1.35),
            column("sponsor", "الجهة الراعية", 23.0, 1.35),
            column("amount", "القيمة الأصلية", 16.0, 0.9),
            column("paid_amount", "المدفوع", 16.0, 0.9),
            column("remaining", "المتبقي", 16.0, 0.9),
            column("status", "الحالة", 13.0, 0.75),
        ],
        "doctor-dues" => vec![
            column("name", "الاسم", 25.0, 1.45),
            column("sessions_count", "عدد الجلسات", 16.0, 0.9),
            column("earned", "المستحق المكتسب", 18.0, 1.05),
            column("paid", "المدفوع", 16.0, 0.9),
            column("remaining", "المتبقي", 16.0, 0.9),
        ],
        "subscriptions" => vec![
            column("patient", "المريض", 22.0, 1.25),
            column("plan_type", "نوع الاشتراك", 16.0, 0.95),
            column("start_date", "تاريخ البداية", 15.0, 0.85),
            column("end_date", "تاريخ النهاية", 15.0, 0.85),
            column("sessions_count", "عدد الجلسات", 13.0, 0.75),
            column("sessions_used", "المستخدمة", 13.0, 0.72),
            column("remaining", "المتبقية", 13.0, 0.72),
            column("remaining_days", "الأيام المتبقية", 14.0, 0.8),
            column("net_total", "القيمة الصافية", 16.0, 0.9),
            column("status", "الحالة", 13.0, 0.7),
        ],
        _ => {
            return Err(ApiError::new(
                axum::http::StatusCode::NOT_FOUND,
                "تعذر العثور على التقرير المطلوب.",
            ));
        }
    };
    Ok(columns)
}

const fn column(
    key: &'static str,
    label: &'static str,
    xlsx_width: f64,
    pdf_weight: f32,
) -> ExportColumn {
    ExportColumn {
        key,
        label,
        xlsx_width,
        pdf_weight,
    }
}

fn report_title(report_type: &str) -> &'static str {
    match report_type {
        "sessions" => "تقرير الجلسات",
        "collections" => "تقرير المقبوضات",
        "debts" => "تقرير الديون",
        "doctor-dues" => "تقرير مستحقات الأطباء",
        "subscriptions" => "تقرير الاشتراكات",
        _ => "تقرير",
    }
}

fn period_label(report: &Value) -> String {
    let meta = report.get("meta").and_then(Value::as_object);
    let from = meta
        .and_then(|value| value.get("from"))
        .and_then(Value::as_str);
    let to = meta
        .and_then(|value| value.get("to"))
        .and_then(Value::as_str);
    match (from, to) {
        (Some(from), Some(to)) => {
            format!("الفترة: من \u{2066}{from}\u{2069} إلى \u{2066}{to}\u{2069}")
        }
        (Some(from), None) => format!("الفترة: من \u{2066}{from}\u{2069}"),
        (None, Some(to)) => format!("الفترة: حتى \u{2066}{to}\u{2069}"),
        (None, None) => "الفترة: كل الفترات".to_string(),
    }
}

fn generated_label(report: &Value) -> String {
    let generated = report
        .pointer("/meta/generated_at")
        .and_then(Value::as_str)
        .unwrap_or("");
    let generated = chrono::DateTime::parse_from_rfc3339(generated)
        .map(|value| value.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|_| generated.to_string());
    format!("تاريخ الإعداد: \u{2066}{generated}\u{2069}")
}

fn display_value(key: &str, value: &Value) -> String {
    if matches!(key, "patient" | "doctor" | "sponsor") {
        return value
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
    }
    if key == "services" {
        return value
            .as_array()
            .map(|services| {
                services
                    .iter()
                    .filter_map(|service| service.get("name").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("، ")
            })
            .unwrap_or_default();
    }
    let raw = match value {
        Value::Null => return String::new(),
        Value::Bool(value) => {
            if *value {
                "نعم".to_string()
            } else {
                "لا".to_string()
            }
        }
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        other => other.to_string(),
    };
    enum_label(&raw).unwrap_or(&raw).to_string()
}

fn enum_label(value: &str) -> Option<&'static str> {
    Some(match value {
        "open" => "مفتوحة",
        "closed" => "مغلقة",
        "cancelled" => "ملغاة",
        "active" => "نشط",
        "completed" => "مكتمل",
        "stopped" => "موقوف",
        "monthly" => "شهري",
        "legacy_package" => "حزمة جلسات قديمة",
        "partial" => "جزئي",
        "paid" => "مسدد",
        "void" => "ملغى",
        "cash" => "نقدًا",
        "transfer" => "تحويل مصرفي",
        "session_payment" => "دفعة جلسة",
        "subscription_payment" => "دفعة اشتراك",
        "wallet_topup" => "شحن رصيد",
        "debt_payment" => "تسديد دين",
        "refund" => "استرجاع",
        "adjustment" => "تصحيح",
        "session" => "جلسة",
        "subscription" => "اشتراك",
        "opening" => "دين افتتاحي",
        _ => return None,
    })
}

fn heading_label(key: &str) -> &str {
    match key {
        "count" => "العدد",
        "service_value" => "قيمة الخدمات",
        "received" => "المقبوضات",
        "refunds" => "الاسترجاعات",
        "net" => "الصافي",
        "original" => "القيمة الأصلية",
        "paid" => "المدفوع",
        "remaining" => "المتبقي",
        "earned" => "المستحق المكتسب",
        "active" => "الاشتراكات النشطة",
        "remaining_sessions" => "الجلسات المتبقية",
        "net_total" => "القيمة الصافية",
        other => other,
    }
}

fn xlsx_error(error: XlsxError) -> ApiError {
    ApiError::internal(format!("Unable to generate the XLSX report: {error}"))
}

fn rgb(red: f32, green: f32, blue: f32) -> Rgb {
    Rgb {
        r: red,
        g: green,
        b: blue,
        icc_profile: None,
    }
}

fn ink() -> PdfColor {
    PdfColor::Rgb(rgb(0.114, 0.114, 0.122))
}

#[cfg(test)]
mod tests {
    use super::{export, glyph_char_map, shape_visual_runs, REGULAR_FONT};
    use printpdf::Pt;
    use serde_json::json;

    fn report() -> serde_json::Value {
        json!({
            "rows": [{
                "opened_at": "2026-11-02T10:00:00+02:00",
                "patient": { "name": "مريض عربي" },
                "doctor": { "name": "طبيب عربي" },
                "services": [{ "name": "علاج يدوي" }],
                "net_total": "80.000",
                "status": "closed"
            }],
            "totals": { "count": 1, "service_value": "80.000" },
            "meta": {
                "type": "sessions",
                "from": null,
                "to": null,
                "generated_at": "2026-11-02T10:00:00+02:00"
            }
        })
    }

    fn visual_report() -> serde_json::Value {
        let rows = (1..=17)
            .map(|index| {
                json!({
                    "opened_at": format!("2026-11-{index:02}T10:30:00+02:00"),
                    "patient": { "name": format!("المريض العربي رقم {index}") },
                    "doctor": { "name": if index % 2 == 0 { "د. أحمد السنوسي" } else { "د. سارة الفيتوري" } },
                    "services": [
                        { "name": "علاج يدوي وتأهيل حركي" },
                        { "name": if index % 3 == 0 { "تمارين علاجية طويلة للاختبار" } else { "جلسة متابعة" } }
                    ],
                    "net_total": format!("{}.500", index * 10),
                    "status": if index % 5 == 0 { "cancelled" } else { "closed" }
                })
            })
            .collect::<Vec<_>>();
        json!({
            "rows": rows,
            "totals": { "count": 17, "service_value": "1530.500" },
            "meta": {
                "type": "sessions",
                "from": "2026-11-01",
                "to": "2026-11-30",
                "generated_at": "2026-11-02T10:00:00+02:00"
            }
        })
    }

    #[test]
    fn binary_exports_have_real_file_signatures() {
        let xlsx = export("sessions", "xlsx", &report()).unwrap();
        assert!(xlsx.bytes.starts_with(b"PK"));
        let pdf = export("sessions", "pdf", &report()).unwrap();
        assert!(pdf.bytes.starts_with(b"%PDF-"));
        assert!(pdf.bytes.len() > 10_000);
    }

    #[test]
    fn arabic_shaping_uses_positioned_font_glyphs() {
        let face = rustybuzz::Face::from_slice(REGULAR_FONT, 0).unwrap();
        let glyph_chars = glyph_char_map(REGULAR_FONT).unwrap();
        let runs = shape_visual_runs(&face, &glyph_chars, "تقرير 2026", Pt(12.0));
        assert!(!runs.is_empty());
        assert!(runs.iter().flat_map(|run| &run.glyphs).count() >= 8);
        assert!(runs
            .iter()
            .flat_map(|run| &run.glyphs)
            .any(|glyph| matches!(glyph.character as u32, 0xFB50..=0xFDFF | 0xFE70..=0xFEFF)));
        assert!(runs.iter().map(|run| run.width).sum::<f32>() > 20.0);
    }

    #[test]
    #[ignore = "writes report artifacts for visual QA"]
    fn writes_visual_review_fixtures() {
        let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root");
        let pdf_directory = workspace.join("tmp/pdfs");
        let spreadsheet_directory = workspace.join("tmp/spreadsheets");
        std::fs::create_dir_all(&pdf_directory).unwrap();
        std::fs::create_dir_all(&spreadsheet_directory).unwrap();

        let report = visual_report();
        let pdf = export("sessions", "pdf", &report).unwrap();
        std::fs::write(pdf_directory.join("sessions-report-rust.pdf"), pdf.bytes).unwrap();

        let xlsx = export("sessions", "xlsx", &report).unwrap();
        std::fs::write(
            spreadsheet_directory.join("sessions-report-rust.xlsx"),
            xlsx.bytes,
        )
        .unwrap();
    }
}
