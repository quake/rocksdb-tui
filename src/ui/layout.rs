use crate::app::{App, Focus};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(frame.area());

    let main_area = chunks[0];
    let status_area = chunks[1];

    // Three column layout
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(30),
            Constraint::Percentage(50),
        ])
        .split(main_area);

    draw_cf_list(frame, app, columns[0]);
    draw_key_list(frame, app, columns[1]);
    draw_value_view(frame, app, columns[2]);
    draw_status_bar(frame, app, status_area);
}

fn draw_cf_list(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::ColumnFamilies;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let items: Vec<ListItem> = app
        .db
        .column_families()
        .iter()
        .map(|cf| ListItem::new(cf.as_str()))
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title("Column Families")
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");

    let mut state = ListState::default();
    state.select(Some(app.cf_index));

    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_key_list(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Keys;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    // Split area for search box and key list
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    // Search box
    let search_style = if app.search_active {
        Style::default().fg(Color::Yellow)
    } else if app.search_prefix.is_some() {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let search_text = if app.search_active {
        app.search_input.clone()
    } else if let Some(ref prefix) = app.search_prefix {
        format!("Filter: {}", String::from_utf8_lossy(prefix))
    } else if app.search_input.is_empty() {
        "Press / to search...".to_string()
    } else {
        app.search_input.clone()
    };
    let search = Paragraph::new(search_text)
        .style(search_style)
        .block(Block::default().title("Search").borders(Borders::ALL));
    frame.render_widget(search, chunks[0]);

    // Key list
    let keys = app.formatted_keys();
    let mut items: Vec<ListItem> = keys.iter().map(|k| ListItem::new(k.as_str())).collect();

    if app.has_more_keys {
        items.push(ListItem::new("▼ more...").style(Style::default().fg(Color::DarkGray)));
    }

    let list = List::new(items)
        .block(
            Block::default()
                .title("Keys")
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");

    let mut state = ListState::default();
    state.select(Some(app.key_index));

    frame.render_stateful_widget(list, chunks[1], &mut state);
}

fn draw_value_view(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Value;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let (content, success) = app.current_value().unwrap_or_default();

    let mut lines = Vec::new();
    if !success {
        lines.push(Line::from(Span::styled(
            "⚠ Parse failed, showing hex",
            Style::default().fg(Color::Yellow),
        )));
        lines.push(Line::from(""));
    }
    for line in content.lines() {
        lines.push(Line::from(line));
    }

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title("Value")
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

fn format_number(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let key_count = if app.search_prefix.is_some() {
        format!("Filtered: {}", app.keys.len())
    } else {
        app.estimate_keys()
            .map(|n| format!("~{}", format_number(n)))
            .unwrap_or_else(|| "?".to_string())
    };

    let status = format!(
        " CF: {} | Keys: {} | Format: {:?} | [?] Help [/] Search [q] Quit ",
        app.current_cf().unwrap_or("none"),
        key_count,
        app.value_format()
    );

    let paragraph = Paragraph::new(status).style(Style::default().bg(Color::DarkGray));
    frame.render_widget(paragraph, area);
}
