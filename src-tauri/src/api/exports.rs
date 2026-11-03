use super::error::{ApiError, ApiResult};
use printpdf::{
    Color as PdfColor, FontId, Mm, Op, ParsedFont, PdfDocument, PdfPage, PdfSaveOptions, Pt,
    RawImage, Rect, Rgb, TextItem, TextMatrix, XObjectId, XObjectTransform,
};
use rust_xlsxwriter::{Color as XlsxColor, Format, FormatAlign, FormatBorder, Workbook, XlsxError};
use serde_json::Value;
use std::collections::HashMap;
use unicode_bidi::{BidiInfo, Level};

const REGULAR_FONT: &[u8] = include_bytes!("../../assets/fonts/DejaVuSans.ttf");
const BOLD_FONT: &[u8] = include_bytes!("../../assets/fonts/DejaVuSans-Bold.ttf");
const CLINIC_LOGO: &[u8] = include_bytes!("../../../public/clinic-logo-128.png");

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
                let value = display_row_value(report_type, column.key, row);
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
                .write_string_with_format(
                    row,
                    0,
                    heading_label(report_type, key),
                    &total_label_format,
                )
                .map_err(xlsx_error)?;
            worksheet
                .write_string_with_format(
                    row,
                    1,
                    &display_value(report_type, key, value),
                    &total_value_format,
                )
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
    let logo_id = RawImage::decode_from_bytes(CLINIC_LOGO, &mut warnings)
        .ok()
        .map(|logo| document.add_image(&logo));
    let fonts = PdfFonts {
        regular_id,
        regular_face,
        regular_glyph_chars: glyph_char_map(REGULAR_FONT)?,
        bold_id,
        bold_face,
        bold_glyph_chars: glyph_char_map(BOLD_FONT)?,
        logo_id,
    };

    let page_width = Mm(210.0).into_pt();
    let page_height = Mm(297.0).into_pt();
    let margin = Pt(34.0);
    let table_width = page_width - margin - margin;
    let row_height = Pt(23.0);
    let header_height = Pt(25.0);
    let rows_per_page = 23usize;
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
        draw_summary_cards(
            &mut ops,
            &fonts,
            report_type,
            totals,
            margin,
            page_width - margin,
            page_height - Pt(116.0),
        );
        draw_text_right(
            &mut ops,
            &fonts.bold_face,
            &fonts.bold_glyph_chars,
            &fonts.bold_id,
            "بيانات التقرير",
            Pt(11.5),
            page_width - margin,
            page_height - Pt(177.0),
            table_width,
            ink(),
        );
        draw_box(
            &mut ops,
            Rect {
                x: margin,
                y: page_height - Pt(183.0),
                width: table_width,
                height: Pt(1.5),
            },
            brand(),
            false,
        );
        let table_top = page_height - Pt(190.0);
        draw_table_row(
            &mut ops,
            &fonts,
            report_type,
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
                    report_type,
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

        draw_page_footer(
            &mut ops,
            &fonts,
            report,
            page_width,
            page_number_label(page_index + 1, page_count),
        );
        pages.push(PdfPage::new(Mm(210.0), Mm(297.0), ops));
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
    logo_id: Option<XObjectId>,
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
    let margin = Pt(34.0);
    let content_width = page_width - margin - margin;
    let clinic_name = clinic_value(report, "clinic_name", "عيادة عين الحياة");
    let clinic_subtitle = clinic_value(report, "clinic_subtitle", "للعلاج الطبيعي والتأهيل");
    let show_logo = report
        .pointer("/meta/clinic/show_logo")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let brand_right = if show_logo {
        if let Some(logo_id) = &fonts.logo_id {
            ops.push(Op::UseXobject {
                id: logo_id.clone(),
                transform: XObjectTransform {
                    translate_x: Some(page_width - margin - Pt(40.0)),
                    translate_y: Some(page_height - Pt(54.0)),
                    dpi: Some(230.0),
                    ..Default::default()
                },
            });
            page_width - margin - Pt(48.0)
        } else {
            page_width - margin
        }
    } else {
        page_width - margin
    };
    draw_text_right(
        ops,
        &fonts.bold_face,
        &fonts.bold_glyph_chars,
        &fonts.bold_id,
        &clinic_name,
        Pt(18.0),
        brand_right,
        page_height - Pt(29.0),
        Pt(235.0),
        brand_dark(),
    );
    draw_text_right(
        ops,
        &fonts.regular_face,
        &fonts.regular_glyph_chars,
        &fonts.regular_id,
        &clinic_subtitle,
        Pt(9.5),
        brand_right,
        page_height - Pt(46.0),
        Pt(235.0),
        muted(),
    );
    draw_text_right(
        ops,
        &fonts.bold_face,
        &fonts.bold_glyph_chars,
        &fonts.bold_id,
        report_title(report_type),
        Pt(15.0),
        margin + Pt(220.0),
        page_height - Pt(29.0),
        Pt(220.0),
        ink(),
    );
    draw_text_right(
        ops,
        &fonts.regular_face,
        &fonts.regular_glyph_chars,
        &fonts.regular_id,
        &format!(
            "{} · صفحة {} من {}",
            generated_label(report),
            page_number,
            page_count
        ),
        Pt(7.5),
        margin + Pt(220.0),
        page_height - Pt(46.0),
        Pt(220.0),
        muted(),
    );
    draw_box(
        ops,
        Rect {
            x: margin,
            y: page_height - Pt(61.0),
            width: content_width,
            height: Pt(2.0),
        },
        brand(),
        false,
    );
    draw_box(
        ops,
        Rect {
            x: margin,
            y: page_height - Pt(73.0),
            width: content_width,
            height: Pt(31.0),
        },
        PdfColor::Rgb(rgb(1.0, 1.0, 1.0)),
        true,
    );
    draw_text_right(
        ops,
        &fonts.regular_face,
        &fonts.regular_glyph_chars,
        &fonts.regular_id,
        "الفترة",
        Pt(7.0),
        page_width - margin - Pt(8.0),
        page_height - Pt(84.0),
        content_width - Pt(16.0),
        muted(),
    );
    draw_text_right(
        ops,
        &fonts.bold_face,
        &fonts.bold_glyph_chars,
        &fonts.bold_id,
        &period_label(report),
        Pt(9.0),
        page_width - margin - Pt(8.0),
        page_height - Pt(98.0),
        content_width - Pt(16.0),
        ink(),
    );
}

fn draw_summary_cards(
    ops: &mut Vec<Op>,
    fonts: &PdfFonts<'_>,
    report_type: &str,
    totals: &serde_json::Map<String, Value>,
    left: Pt,
    right: Pt,
    top: Pt,
) {
    let keys = total_keys(report_type)
        .iter()
        .filter(|key| totals.contains_key(**key))
        .copied()
        .collect::<Vec<_>>();
    if keys.is_empty() {
        return;
    }
    let gap = Pt(6.0);
    let available = right - left - Pt(gap.0 * keys.len().saturating_sub(1) as f32);
    let width = Pt(available.0 / keys.len() as f32);
    let height = Pt(43.0);
    let mut card_right = right;
    for key in keys {
        let rect = Rect {
            x: card_right - width,
            y: top,
            width,
            height,
        };
        draw_box(ops, rect.clone(), tint(), true);
        draw_text_right(
            ops,
            &fonts.regular_face,
            &fonts.regular_glyph_chars,
            &fonts.regular_id,
            heading_label(report_type, key),
            Pt(7.0),
            rect.x + rect.width - Pt(6.0),
            top - Pt(13.0),
            rect.width - Pt(12.0),
            muted(),
        );
        if let Some(value) = totals.get(key) {
            draw_text_right(
                ops,
                &fonts.bold_face,
                &fonts.bold_glyph_chars,
                &fonts.bold_id,
                &display_value(report_type, key, value),
                Pt(10.5),
                rect.x + rect.width - Pt(6.0),
                top - Pt(31.0),
                rect.width - Pt(12.0),
                ink(),
            );
        }
        card_right = rect.x - gap;
    }
}

fn draw_page_footer(
    ops: &mut Vec<Op>,
    fonts: &PdfFonts<'_>,
    report: &Value,
    page_width: Pt,
    page_label: String,
) {
    let margin = Pt(34.0);
    let content_width = page_width - margin - margin;
    draw_box(
        ops,
        Rect {
            x: margin,
            y: Pt(43.0),
            width: content_width,
            height: Pt(0.8),
        },
        print_border(),
        false,
    );
    let contact = [
        clinic_value(report, "address", ""),
        clinic_value(report, "phone", ""),
    ]
    .into_iter()
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>()
    .join(" · ");
    draw_text_right(
        ops,
        &fonts.regular_face,
        &fonts.regular_glyph_chars,
        &fonts.regular_id,
        if contact.is_empty() { "—" } else { &contact },
        Pt(7.0),
        page_width - margin,
        Pt(29.0),
        content_width - Pt(80.0),
        muted(),
    );
    draw_text_right(
        ops,
        &fonts.regular_face,
        &fonts.regular_glyph_chars,
        &fonts.regular_id,
        &page_label,
        Pt(7.0),
        margin + Pt(72.0),
        Pt(29.0),
        Pt(72.0),
        muted(),
    );
}

fn page_number_label(page_number: usize, page_count: usize) -> String {
    format!("صفحة {page_number} من {page_count}")
}

#[allow(clippy::too_many_arguments)]
fn draw_table_row(
    ops: &mut Vec<Op>,
    fonts: &PdfFonts<'_>,
    report_type: &str,
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
                brand()
            } else {
                PdfColor::Rgb(rgb(1.0, 1.0, 1.0))
            },
            true,
        );
        let text = if header {
            column.label.to_string()
        } else {
            row.map(|value| display_row_value(report_type, column.key, value))
                .unwrap_or_default()
        };
        let (face, glyph_chars, font_id, size) = if header {
            (
                &fonts.bold_face,
                &fonts.bold_glyph_chars,
                &fonts.bold_id,
                Pt(7.4),
            )
        } else {
            (
                &fonts.regular_face,
                &fonts.regular_glyph_chars,
                &fonts.regular_id,
                Pt(7.1),
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
            if header {
                PdfColor::Rgb(rgb(1.0, 1.0, 1.0))
            } else {
                ink()
            },
        );
        right = rect.x;
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
            col: print_border(),
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
            column("opened_at", "التاريخ", 22.0, 1.15),
            column("patient", "المريض", 24.0, 1.3),
            column("doctor", "الطبيب", 21.0, 1.15),
            column("services", "الخدمات", 38.0, 1.9),
            column("net_total", "القيمة", 16.0, 1.05),
            column("status", "الحالة", 13.0, 0.9),
        ],
        "collections" => vec![
            column("created_at", "التاريخ", 22.0, 1.2),
            column("type", "النوع", 22.0, 1.25),
            column("patient", "المريض", 25.0, 1.45),
            column("method", "الطريقة", 18.0, 1.0),
            column("amount", "المبلغ", 16.0, 0.9),
        ],
        "debts" => vec![
            column("id", "المرجع", 10.0, 0.72),
            column("patient", "المريض", 23.0, 1.3),
            column("sponsor", "المسؤول", 23.0, 1.15),
            column("amount", "الأصل", 16.0, 1.0),
            column("paid_amount", "المسدد", 16.0, 1.0),
            column("remaining", "المتبقي", 16.0, 1.0),
            column("status", "الحالة", 13.0, 1.0),
        ],
        "doctor-dues" => vec![
            column("name", "الطبيب", 25.0, 1.45),
            column("sessions_count", "الجلسات", 16.0, 0.9),
            column("earned", "المكتسب", 18.0, 1.05),
            column("paid", "المصروف", 16.0, 1.0),
            column("remaining", "المتبقي", 16.0, 1.05),
        ],
        "subscriptions" => vec![
            column("patient", "المريض", 22.0, 1.35),
            column("plan_type", "النوع", 16.0, 0.9),
            column("start_date", "البداية", 15.0, 1.0),
            column("subscription_end", "النهاية", 15.0, 1.0),
            column("subscription_remaining", "المتبقي", 17.0, 1.15),
            column("net_total", "القيمة", 16.0, 0.95),
            column("status", "الحالة", 13.0, 0.9),
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
        (Some(from), Some(to)) => format!(
            "من {} إلى {}",
            format_report_date(from),
            format_report_date(to)
        ),
        (Some(from), None) => format!("من {}", format_report_date(from)),
        (None, Some(to)) => format!("حتى {}", format_report_date(to)),
        (None, None) => "كل الفترات".to_string(),
    }
}

fn generated_label(report: &Value) -> String {
    let generated = report
        .pointer("/meta/generated_at")
        .and_then(Value::as_str)
        .unwrap_or("");
    let generated = chrono::DateTime::parse_from_rfc3339(generated)
        .map(|value| {
            format!(
                "{}، {}",
                format_naive_date(value.date_naive()),
                value.format("%H:%M")
            )
        })
        .unwrap_or_else(|_| generated.to_string());
    format!("أُعد في {generated}")
}

fn display_row_value(report_type: &str, key: &str, row: &Value) -> String {
    if key == "subscription_end" {
        return row
            .get("end_date")
            .filter(|value| !value.is_null())
            .or_else(|| row.get("expected_end_date"))
            .map(|value| display_value(report_type, "end_date", value))
            .unwrap_or_else(|| "—".to_string());
    }
    if key == "subscription_remaining" {
        let plan_type = row.get("plan_type").and_then(Value::as_str).unwrap_or("");
        let (value_key, unit) = if plan_type == "monthly" {
            ("remaining_days", "يوم")
        } else {
            ("remaining", "جلسة")
        };
        let value = row
            .get(value_key)
            .map(scalar_text)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "0".to_string());
        return format!("\u{2066}{value}\u{2069} {unit}");
    }
    if key == "sponsor" && row.get(key).is_none_or(Value::is_null) {
        return "المريض".to_string();
    }
    row.get(key)
        .map(|value| display_value(report_type, key, value))
        .unwrap_or_else(|| "—".to_string())
}

fn display_value(report_type: &str, key: &str, value: &Value) -> String {
    if matches!(key, "patient" | "doctor" | "sponsor") {
        return value
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("—")
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
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "—".to_string());
    }
    let raw = match value {
        Value::Null => return "—".to_string(),
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
    if matches!(key, "opened_at" | "created_at" | "start_date" | "end_date") {
        return format_report_date(&raw);
    }
    if is_money_key(key) {
        return format_money(&raw);
    }
    if key == "id" {
        return format!("#\u{2066}{raw}\u{2069}");
    }
    enum_label(report_type, key, &raw)
        .unwrap_or(&raw)
        .to_string()
}

fn enum_label(report_type: &str, key: &str, value: &str) -> Option<&'static str> {
    Some(match (key, value) {
        ("status", "open") if report_type == "sessions" => "مفتوحة",
        ("status", "open") => "مفتوح",
        ("status", "closed") => "مغلقة",
        ("status", "cancelled") if report_type == "subscriptions" => "ملغي",
        ("status", "cancelled") => "ملغاة",
        ("status", "active") => "نشط",
        ("status", "completed") => "مكتمل",
        ("status", "stopped") => "موقوف",
        ("status", "partial") => "جزئي",
        ("status", "paid") => "مسدد",
        ("status", "void") => "ملغى",
        ("plan_type", "monthly") => "شهري",
        ("plan_type", "legacy_package") => "حزمة جلسات",
        ("method", "cash") => "نقدًا",
        ("method", "transfer") => "تحويل مصرفي",
        ("type", "session_payment") => "دفعة جلسة",
        ("type", "subscription_payment") => "دفعة اشتراك",
        ("type", "wallet_topup") => "شحن رصيد",
        ("type", "debt_payment") => "تسديد دين",
        ("type", "refund") => "استرجاع",
        ("type", "adjustment") => "تسوية",
        ("kind", "session") => "جلسة",
        ("kind", "subscription") => "اشتراك",
        ("kind", "opening") => "دين افتتاحي",
        (_, "open") => "مفتوح",
        (_, "closed") => "مغلقة",
        (_, "cancelled") => "ملغاة",
        (_, "active") => "نشط",
        _ => return None,
    })
}

fn heading_label<'a>(report_type: &str, key: &'a str) -> &'a str {
    match (report_type, key) {
        ("sessions", "count") => "عدد الجلسات",
        ("subscriptions", "count") => "عدد الاشتراكات",
        (_, "service_value") => "قيمة الخدمات",
        (_, "received") => "إجمالي المقبوضات",
        (_, "refunds") => "إجمالي الاسترجاعات",
        (_, "net") => "صافي المقبوضات",
        (_, "original") => "إجمالي الديون",
        ("doctor-dues", "paid") => "إجمالي المصروف",
        (_, "paid") => "إجمالي المسدد",
        (_, "remaining") => "إجمالي المتبقي",
        (_, "earned") => "إجمالي المستحق المكتسب",
        (_, "active") => "الاشتراكات النشطة",
        (_, "remaining_sessions") => "الجلسات المتبقية",
        (_, "net_total") => "قيمة الاشتراكات",
        (_, other) => other,
    }
}

fn total_keys(report_type: &str) -> &'static [&'static str] {
    match report_type {
        "sessions" => &["count", "service_value"],
        "collections" => &["received", "refunds", "net"],
        "debts" => &["original", "paid", "remaining"],
        "doctor-dues" => &["earned", "paid", "remaining"],
        "subscriptions" => &["count", "active", "remaining_sessions", "net_total"],
        _ => &[],
    }
}

fn is_money_key(key: &str) -> bool {
    matches!(
        key,
        "service_value"
            | "received"
            | "refunds"
            | "net"
            | "original"
            | "paid"
            | "remaining"
            | "earned"
            | "net_total"
            | "amount"
            | "paid_amount"
    )
}

fn format_money(value: &str) -> String {
    let number = trim_decimal(value);
    format!("\u{2066}{number}\u{2069} د.ل")
}

fn trim_decimal(value: &str) -> String {
    let value = value.trim();
    let Some((whole, fraction)) = value.split_once('.') else {
        return value.to_string();
    };
    let fraction = fraction.trim_end_matches('0');
    if fraction.is_empty() {
        whole.to_string()
    } else {
        format!("{whole}.{fraction}")
    }
}

fn scalar_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        other => other.to_string(),
    }
}

fn format_report_date(value: &str) -> String {
    let date = chrono::DateTime::parse_from_rfc3339(value)
        .map(|value| value.date_naive())
        .or_else(|_| chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d"));
    date.map(format_naive_date)
        .unwrap_or_else(|_| value.to_string())
}

fn format_naive_date(value: chrono::NaiveDate) -> String {
    use chrono::Datelike;
    let months = [
        "يناير",
        "فبراير",
        "مارس",
        "أبريل",
        "مايو",
        "يونيو",
        "يوليو",
        "أغسطس",
        "سبتمبر",
        "أكتوبر",
        "نوفمبر",
        "ديسمبر",
    ];
    let month = months[value.month0() as usize];
    format!("{} {month} {}", value.day(), value.year())
}

fn clinic_value(report: &Value, key: &str, fallback: &str) -> String {
    report
        .pointer(&format!("/meta/clinic/{key}"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
        .to_string()
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

fn brand() -> PdfColor {
    PdfColor::Rgb(rgb(0.0, 0.471, 0.31))
}

fn brand_dark() -> PdfColor {
    PdfColor::Rgb(rgb(0.0, 0.412, 0.294))
}

fn tint() -> PdfColor {
    PdfColor::Rgb(rgb(0.918, 0.957, 0.937))
}

fn muted() -> PdfColor {
    PdfColor::Rgb(rgb(0.337, 0.384, 0.467))
}

fn print_border() -> PdfColor {
    PdfColor::Rgb(rgb(0.788, 0.831, 0.886))
}

#[cfg(test)]
mod tests {
    use super::{
        columns, display_row_value, export, glyph_char_map, shape_visual_runs, CLINIC_LOGO,
        REGULAR_FONT,
    };
    use printpdf::{Pt, RawImage};
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
        let rows = (1..=45)
            .map(|index| {
                let day = ((index - 1) % 28) + 1;
                json!({
                    "opened_at": format!("2026-11-{day:02}T10:30:00+02:00"),
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
            "totals": { "count": 45, "service_value": "10350.500" },
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
    fn bundled_clinic_logo_decodes_for_pdf_headers() {
        RawImage::decode_from_bytes(CLINIC_LOGO, &mut Vec::new())
            .expect("the bundled clinic logo must decode");
    }

    #[test]
    fn report_exports_use_the_same_compact_columns_and_arabic_values() {
        let expectations = [
            (
                "sessions",
                vec!["التاريخ", "المريض", "الطبيب", "الخدمات", "القيمة", "الحالة"],
            ),
            (
                "collections",
                vec!["التاريخ", "النوع", "المريض", "الطريقة", "المبلغ"],
            ),
            (
                "debts",
                vec![
                    "المرجع",
                    "المريض",
                    "المسؤول",
                    "الأصل",
                    "المسدد",
                    "المتبقي",
                    "الحالة",
                ],
            ),
            (
                "doctor-dues",
                vec!["الطبيب", "الجلسات", "المكتسب", "المصروف", "المتبقي"],
            ),
            (
                "subscriptions",
                vec![
                    "المريض",
                    "النوع",
                    "البداية",
                    "النهاية",
                    "المتبقي",
                    "القيمة",
                    "الحالة",
                ],
            ),
        ];
        for (report_type, expected) in expectations {
            assert_eq!(
                columns(report_type)
                    .unwrap()
                    .iter()
                    .map(|column| column.label)
                    .collect::<Vec<_>>(),
                expected
            );
        }

        let subscription = json!({
            "plan_type": "monthly",
            "remaining_days": 8,
            "end_date": null,
            "expected_end_date": "2026-11-30"
        });
        assert_eq!(
            display_row_value("subscriptions", "subscription_remaining", &subscription),
            "\u{2066}8\u{2069} يوم"
        );
        assert_eq!(
            display_row_value("subscriptions", "subscription_end", &subscription),
            "30 نوفمبر 2026"
        );
        assert_eq!(
            display_row_value("debts", "sponsor", &json!({ "sponsor": null })),
            "المريض"
        );
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
