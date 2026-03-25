use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Axis, Bar, BarChart, BarGroup, Block, Borders, Chart, Dataset, Paragraph};
use tui4kas_core::rpc::types::{format_number, sompi_to_kas};

use crate::analytics::AggregatedView;
use crate::app::{App, TimeWindow, ViewMode};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if !app.has_direct_url() {
        let block = Block::default().borders(Borders::ALL).title(Span::styled(
            " Analytics ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        let msg = Paragraph::new(Line::from(Span::styled(
            " Disabled when using Kaspa PNN via Resolver",
            Style::default().fg(Color::DarkGray),
        )))
        .block(block);
        frame.render_widget(msg, area);
        return;
    }

    // Determine available vertical space for banners vs panels
    let mut banner_lines: Vec<Line> = Vec::new();

    // Sync progress banner
    if let Some(ref progress) = app.analytics.sync_progress {
        let range = progress.tip_daa.saturating_sub(progress.start_daa);
        let done = progress.last_daa.saturating_sub(progress.start_daa);
        let pct = if range > 0 {
            (done as f64 / range as f64 * 100.0).min(100.0)
        } else {
            0.0
        };
        let mode = if progress.from_pruning_point {
            "from pruning point"
        } else {
            "from current"
        };
        banner_lines.push(Line::from(Span::styled(
            format!(
                " Analyzing {} — DAA {}/{} ({:.1}%) | last analyzed: {}",
                mode,
                format_number(progress.start_daa),
                format_number(progress.tip_daa),
                pct,
                format_number(progress.last_daa),
            ),
            Style::default().fg(Color::Yellow),
        )));
    }

    // Reorg notification
    if let Some(ref msg) = app.analytics.reorg_notification {
        banner_lines.push(Line::from(Span::styled(
            format!(" ⚠ {} (press Esc to dismiss)", msg),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
    }

    let main_area = if !banner_lines.is_empty() {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(banner_lines.len() as u16),
                Constraint::Min(0),
            ])
            .split(area);
        frame.render_widget(Paragraph::new(banner_lines), chunks[0]);
        chunks[1]
    } else {
        area
    };

    // Use cached views (refreshed by analytics streaming task)
    let default_views: [AggregatedView; crate::app::ANALYTICS_PANEL_COUNT] = Default::default();
    let views = app
        .analytics
        .cached_views
        .as_ref()
        .unwrap_or(&default_views);

    // Layout: 3 rows — top 2 columns, mid 2 columns, bottom 2 columns
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(main_area);

    let top_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[0]);

    let mid_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);

    let bottom_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[2]);

    let panel_areas = [
        top_cols[0],
        top_cols[1],
        mid_cols[0],
        mid_cols[1],
        bottom_cols[0],
        bottom_cols[1],
    ];

    let panel_names = [
        "Fees",
        "Tx Summary",
        "Top Senders",
        "Top Receivers",
        "Protocols",
        "Tx Counts",
    ];

    for (i, (&panel_area, name)) in panel_areas.iter().zip(panel_names.iter()).enumerate() {
        let is_focused = app.analytics.focus == i;
        let border_style = if is_focused {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let tw = app.analytics.time_windows[i].label();
        let title = format!(" {} [{}] ", name, tw);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(Span::styled(
                title,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));

        let inner = block.inner(panel_area);
        frame.render_widget(block, panel_area);

        if app.analytics.cached_views.is_none() {
            frame.render_widget(
                Paragraph::new(Span::styled(
                    " Collecting data...",
                    Style::default().fg(Color::DarkGray),
                )),
                inner,
            );
            continue;
        }

        match (i, app.analytics.view_modes[i]) {
            (0, ViewMode::Table) => render_fees_table(frame, inner, &views[0], app),
            (0, ViewMode::Chart) => render_mass_chart(frame, inner, &views[0]),
            (1, ViewMode::Table) => {
                render_tx_table(frame, inner, &views[1], app.analytics.time_windows[1])
            }
            (1, ViewMode::Chart) => render_tx_chart(frame, inner, &views[1]),
            (2, ViewMode::Table) => render_address_table(frame, inner, &views[2], true),
            (2, ViewMode::Chart) => render_address_chart(frame, inner, &views[2], "Senders"),
            (3, ViewMode::Table) => render_address_table(frame, inner, &views[3], false),
            (3, ViewMode::Chart) => render_address_chart(frame, inner, &views[3], "Receivers"),
            (4, ViewMode::Table) => render_protocol_table(frame, inner, &views[4]),
            (4, ViewMode::Chart) => render_protocol_chart(frame, inner, &views[4]),
            (5, _) => render_tx_count_bars(frame, inner, &views[5], app.analytics.time_windows[5]),
            _ => {}
        }
    }
}

// --- Table Renderers ---

fn render_fees_table(frame: &mut Frame, area: Rect, view: &AggregatedView, app: &App) {
    let mut lines = vec![
        Line::from(vec![
            Span::styled(" Total Fees:    ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{:.8} KAS", sompi_to_kas(view.total_fees))),
        ]),
        Line::from(vec![
            Span::styled(" Avg Fee:       ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{:.8} KAS", sompi_to_kas(view.avg_fee as u64))),
        ]),
    ];

    // Fee estimate buckets from node
    if let Some(ref fee) = app.node.fee_estimate {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(" Priority Fee:  ", Style::default().fg(Color::DarkGray)),
            Span::raw(&fee.priority_bucket),
        ]));
        if let Some(normal) = fee.normal_buckets.first() {
            lines.push(Line::from(vec![
                Span::styled(" Normal Fee:    ", Style::default().fg(Color::DarkGray)),
                Span::raw(normal),
            ]));
        }
        if let Some(low) = fee.low_buckets.first() {
            lines.push(Line::from(vec![
                Span::styled(" Low Fee:       ", Style::default().fg(Color::DarkGray)),
                Span::raw(low),
            ]));
        }
    }

    frame.render_widget(Paragraph::new(lines), area);
}

fn render_tx_table(frame: &mut Frame, area: Rect, view: &AggregatedView, tw: TimeWindow) {
    let tps = view.tx_count as f64 / tw.seconds();
    let lines = vec![
        Line::from(vec![
            Span::styled(
                " Total Transactions: ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw(format_number(view.tx_count as u64)),
        ]),
        Line::from(vec![
            Span::styled(
                " TPS:                ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw(format!("{:.2}", tps)),
        ]),
        Line::from(vec![
            Span::styled(
                " Avg Mass:           ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw(format!("{:.2}", view.avg_mass)),
        ]),
        Line::from(vec![
            Span::styled(
                " Unique Senders:     ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw(view.top_senders.len().to_string()),
        ]),
        Line::from(vec![
            Span::styled(
                " Unique Receivers:   ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw(view.top_receivers.len().to_string()),
        ]),
    ];
    frame.render_widget(Paragraph::new(lines), area);
}

fn render_protocol_table(frame: &mut Frame, area: Rect, view: &AggregatedView) {
    if view.protocol_counts.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                " No protocol activity detected",
                Style::default().fg(Color::DarkGray),
            )),
            area,
        );
        return;
    }

    let fee_map: std::collections::HashMap<_, _> = view.protocol_fees.iter().cloned().collect();

    let mut lines = vec![Line::from(vec![
        Span::styled(
            "  Protocol",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
        Span::styled(
            "Txs",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("       "),
        Span::styled(
            "Fees (KAS)",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ])];

    for (proto, count) in &view.protocol_counts {
        let fees = fee_map.get(proto).copied().unwrap_or(0);
        lines.push(Line::from(vec![
            Span::raw(format!("  {:<11} ", proto.label())),
            Span::raw(format!("{:>8}  ", format_number(*count as u64))),
            Span::raw(format!("{:>12.4}", sompi_to_kas(fees))),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), area);
}

fn render_address_table(frame: &mut Frame, area: Rect, view: &AggregatedView, is_senders: bool) {
    let entries = if is_senders {
        &view.top_senders
    } else {
        &view.top_receivers
    };

    if entries.is_empty() {
        let label = if is_senders { "sender" } else { "receiver" };
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!(" No {} data available", label),
                Style::default().fg(Color::DarkGray),
            )),
            area,
        );
        return;
    }

    let mut lines = vec![Line::from(vec![
        Span::styled(
            "  Address",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("                                         "),
        Span::styled(
            "Tx Count",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ])];

    for (addr, count) in entries {
        lines.push(Line::from(vec![
            Span::raw(format!("  {:<45} ", addr)),
            Span::raw(format_number(*count as u64)),
        ]));
    }

    frame.render_widget(Paragraph::new(lines), area);
}

// --- Chart Renderers ---

fn render_mass_chart(frame: &mut Frame, area: Rect, view: &AggregatedView) {
    if view.mass_over_time.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                " No fee data for chart",
                Style::default().fg(Color::DarkGray),
            )),
            area,
        );
        return;
    }

    let data = &view.mass_over_time;
    let (x_min, x_max, y_min, y_max) = compute_bounds(data);

    let dataset = Dataset::default()
        .name("Avg Mass")
        .marker(Marker::Braille)
        .style(Style::default().fg(Color::Cyan))
        .data(data);

    let chart = Chart::new(vec![dataset])
        .x_axis(
            Axis::default()
                .style(Style::default().fg(Color::DarkGray))
                .bounds([x_min, x_max]),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(Color::DarkGray))
                .bounds([y_min, y_max])
                .labels(vec![
                    Span::raw(format!("{:.0}", y_min)),
                    Span::raw(format!("{:.0}", y_max)),
                ]),
        );

    frame.render_widget(chart, area);
}

fn render_tx_chart(frame: &mut Frame, area: Rect, view: &AggregatedView) {
    if view.tx_over_time.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                " No tx data for chart",
                Style::default().fg(Color::DarkGray),
            )),
            area,
        );
        return;
    }

    let data = &view.tx_over_time;
    let (x_min, x_max, y_min, y_max) = compute_bounds(data);

    let dataset = Dataset::default()
        .name("Tx Count")
        .marker(Marker::Braille)
        .style(Style::default().fg(Color::Green))
        .data(data);

    let chart = Chart::new(vec![dataset])
        .x_axis(
            Axis::default()
                .style(Style::default().fg(Color::DarkGray))
                .bounds([x_min, x_max]),
        )
        .y_axis(
            Axis::default()
                .style(Style::default().fg(Color::DarkGray))
                .bounds([y_min, y_max])
                .labels(vec![
                    Span::raw(format!("{:.0}", y_min)),
                    Span::raw(format!("{:.0}", y_max)),
                ]),
        );

    frame.render_widget(chart, area);
}

fn render_protocol_chart(frame: &mut Frame, area: Rect, view: &AggregatedView) {
    if view.protocol_counts.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                " No protocol data for chart",
                Style::default().fg(Color::DarkGray),
            )),
            area,
        );
        return;
    }

    let colors = [
        Color::Cyan,
        Color::Green,
        Color::Yellow,
        Color::Magenta,
        Color::Red,
        Color::Blue,
    ];

    let bars: Vec<Bar> = view
        .protocol_counts
        .iter()
        .enumerate()
        .map(|(i, (proto, count))| {
            Bar::default()
                .label(proto.label().into())
                .value(*count as u64)
                .style(Style::default().fg(colors[i % colors.len()]))
        })
        .collect();

    let bar_chart = BarChart::default()
        .data(BarGroup::default().bars(&bars))
        .bar_width((area.width as usize / (view.protocol_counts.len().max(1) + 1)).max(3) as u16)
        .bar_gap(1);

    frame.render_widget(bar_chart, area);
}

fn render_address_chart(frame: &mut Frame, area: Rect, view: &AggregatedView, label: &str) {
    let entries = if label == "Senders" {
        &view.top_senders
    } else {
        &view.top_receivers
    };

    if entries.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                format!(" No {} data for chart", label.to_lowercase()),
                Style::default().fg(Color::DarkGray),
            )),
            area,
        );
        return;
    }

    let top10: Vec<_> = entries.iter().take(10).collect();

    let bars: Vec<Bar> = top10
        .iter()
        .map(|(addr, count)| {
            let short = if addr.len() > 12 {
                format!("{}…", &addr[..11])
            } else {
                addr.clone()
            };
            Bar::default()
                .label(short.into())
                .value(*count as u64)
                .style(Style::default().fg(Color::Cyan))
        })
        .collect();

    let bar_chart = BarChart::default()
        .data(BarGroup::default().bars(&bars))
        .bar_width((area.width as usize / (top10.len().max(1) + 1)).max(3) as u16)
        .bar_gap(1);

    frame.render_widget(bar_chart, area);
}

fn render_tx_count_bars(frame: &mut Frame, area: Rect, view: &AggregatedView, tw: TimeWindow) {
    if view.tx_over_time.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled(
                " No transaction data yet",
                Style::default().fg(Color::DarkGray),
            )),
            area,
        );
        return;
    }

    let period_label = match tw {
        TimeWindow::OneMin => "sec",
        TimeWindow::FifteenMin => "min",
        TimeWindow::ThirtyMin => "min",
        TimeWindow::OneHour => "min",
        TimeWindow::SixHour => "10m",
        TimeWindow::TwelveHour => "10m",
        TimeWindow::TwentyFourHour => "10m",
    };

    let data = &view.tx_over_time;
    let max_bars = (area.width as usize / 4).max(1);
    let skip = data.len().saturating_sub(max_bars);
    let visible: Vec<_> = data.iter().skip(skip).collect();

    let bars: Vec<Bar> = visible
        .iter()
        .enumerate()
        .map(|(i, (_ts, count))| {
            let label = if visible.len() <= 20 || i % (visible.len() / 10).max(1) == 0 {
                format!("{}", i + 1)
            } else {
                String::new()
            };
            Bar::default()
                .label(label.into())
                .value(*count as u64)
                .style(Style::default().fg(Color::Green))
        })
        .collect();

    if bars.is_empty() {
        return;
    }

    let bar_width = ((area.width as usize) / (bars.len() + 1)).clamp(1, 8) as u16;

    let bar_chart = BarChart::default()
        .data(BarGroup::default().bars(&bars))
        .bar_width(bar_width)
        .bar_gap(if bar_width > 1 { 1 } else { 0 });

    // Render period label at bottom-left, then bar chart above
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    frame.render_widget(bar_chart, inner[0]);
    frame.render_widget(
        Paragraph::new(Span::styled(
            format!(" per {}", period_label),
            Style::default().fg(Color::DarkGray),
        )),
        inner[1],
    );
}

// --- Helpers ---

fn compute_bounds(data: &[(f64, f64)]) -> (f64, f64, f64, f64) {
    let x_min = data.iter().map(|(x, _)| *x).fold(f64::INFINITY, f64::min);
    let x_max = data
        .iter()
        .map(|(x, _)| *x)
        .fold(f64::NEG_INFINITY, f64::max);
    let y_min = data.iter().map(|(_, y)| *y).fold(f64::INFINITY, f64::min);
    let y_max = data
        .iter()
        .map(|(_, y)| *y)
        .fold(f64::NEG_INFINITY, f64::max);

    // Ensure non-zero ranges
    let y_max = if (y_max - y_min).abs() < f64::EPSILON {
        y_min + 1.0
    } else {
        y_max
    };
    let x_max = if (x_max - x_min).abs() < f64::EPSILON {
        x_min + 1.0
    } else {
        x_max
    };

    (x_min, x_max, y_min, y_max)
}
