use crate::app::{App, Focus};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

pub fn draw(frame: &mut Frame, app: &mut App) {
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

fn draw_key_list(frame: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Keys;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    // Get formatted key for tooltip (before splitting area)
    let formatted_key = app.formatted_current_key();
    let has_tooltip = formatted_key.is_some();

    // Split area for search box, optional tooltip, and key list
    let constraints = if has_tooltip {
        vec![
            Constraint::Length(3), // Search
            Constraint::Length(3), // Decoded Key tooltip
            Constraint::Min(0),    // Keys
        ]
    } else {
        vec![
            Constraint::Length(3), // Search
            Constraint::Min(0),    // Keys
        ]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    // Search box
    let search_style = if app.search_active {
        Style::default().fg(Color::Yellow)
    } else if app.search_prefix.is_some() {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let search_text = if app.search_input.is_empty() && !app.search_active {
        "/ to search (0x for hex)".to_string()
    } else {
        app.search_input.clone()
    };
    let search = Paragraph::new(search_text)
        .style(search_style)
        .block(Block::default().title("Search").borders(Borders::ALL));
    frame.render_widget(search, chunks[0]);

    // Tooltip and key list areas depend on whether tooltip exists
    let (tooltip_area, keys_area) = if has_tooltip {
        (Some(chunks[1]), chunks[2])
    } else {
        (None, chunks[1])
    };

    // Draw tooltip for formatted key (above key list)
    if let Some(formatted) = formatted_key {
        let tooltip = Paragraph::new(formatted)
            .style(Style::default().fg(Color::Cyan))
            .block(
                Block::default()
                    .title("Decoded Key")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            );
        frame.render_widget(tooltip, tooltip_area.unwrap());
    }

    // Key list
    let keys = app.formatted_keys();

    let (items, show_empty_message): (Vec<ListItem>, Option<&str>) = if keys.is_empty() {
        if app.search_prefix.is_some() {
            (vec![], Some("No matching keys found"))
        } else {
            (vec![], Some("No keys in this column family"))
        }
    } else {
        let mut items: Vec<ListItem> = keys.iter().map(|k| ListItem::new(k.as_str())).collect();
        if app.has_more_keys {
            items.push(ListItem::new("▼ more...").style(Style::default().fg(Color::DarkGray)));
        }
        (items, None)
    };

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
    if !keys.is_empty() {
        state.select(Some(app.key_index));
    }

    frame.render_stateful_widget(list, keys_area, &mut state);

    // Show empty message if needed
    if let Some(msg) = show_empty_message {
        let inner = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(keys_area);
        let empty_msg = Paragraph::new(msg)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(empty_msg, inner[1]);
    }
}

fn draw_value_view(frame: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Value;
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let lines: Vec<Line> = if app.keys.is_empty() {
        let msg = if app.search_prefix.is_some() {
            "No matching keys"
        } else {
            "No keys to display"
        };
        vec![Line::from(Span::styled(
            msg,
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
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
            lines.push(Line::from(line.to_string()));
        }
        lines
    };

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
    // Show status message if present, otherwise show normal status bar
    if let Some(ref msg) = app.status_message {
        let paragraph = Paragraph::new(format!(" {} ", msg))
            .style(Style::default().fg(Color::Black).bg(Color::Green));
        frame.render_widget(paragraph, area);
        return;
    }

    let key_count = if app.search_prefix.is_some() {
        format!("Filtered: {}", app.keys.len())
    } else {
        app.estimate_keys()
            .map(|n| format!("~{}", format_number(n)))
            .unwrap_or_else(|| "?".to_string())
    };

    let status = format!(
        " CF: {} | Keys: {} | Format: {:?} | [?] Help [/] Search [r] Refresh [q] Quit ",
        app.current_cf().unwrap_or("none"),
        key_count,
        app.value_format()
    );

    let paragraph = Paragraph::new(status).style(Style::default().bg(Color::DarkGray));
    frame.render_widget(paragraph, area);
}
