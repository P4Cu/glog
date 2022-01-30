use crate::{app::{self, App, Entry}};
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
    Frame,
};

// TODO: allow to scroll left/right on very long lines

fn log_line<'a>(entry: &'a Entry, app: &app::App) -> Spans<'a> {
    // TODO: style as struct
    let hash_style = Style::default().fg(Color::Yellow);
    let heads_style = Style::default().fg(Color::Green);
    let head_style = heads_style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
    let remotes_style = Style::default().fg(Color::Red);
    let tags_style = Style::default().fg(Color::Yellow);
    let parantheses_style = Style::default().fg(Color::Yellow);
    let subject_style = Style::default().fg(Color::White);
    let author_date_style = Style::default().fg(Color::DarkGray);

    let mut spans = Vec::new();
    if entry.selected() {
        spans.push(Span::raw("➡️ "));
    } else if app.log.has_selected() {
        spans.push(Span::raw("  "));
    }
    spans.push(Span::raw(entry.git.graph.clone()));
    spans.push(Span::styled(
        entry.git.hash.chars().take(8).collect::<String>(),
        hash_style,
    ));
    spans.push(Span::raw(" "));
    if let Some(refs) = &entry.git.refs {
        // build iterator of Spans
        let mut v = Vec::new();
        if let Some(head) = &refs.head {
            v.push(Span::styled(head, head_style));
        }
        v.extend(refs.heads.iter().map(|v| Span::styled(v, heads_style)));
        v.extend(
            refs.remotes
                .iter()
                .filter(|v| v.as_str() != "origin/HEAD")
                .map(|v| Span::styled(v, remotes_style)),
        );
        v.extend(refs.tags.iter().map(|v| Span::styled(v, tags_style)));

        if !v.is_empty() {
            spans.push(Span::styled("(", parantheses_style));
            spans.extend(v.iter().skip(1).fold(vec![v[0].clone()], |mut acc, v| {
                acc.push(Span::raw(", "));
                acc.push(v.clone());
                acc
            }));
            spans.push(Span::styled(") ", parantheses_style));
        }
    }
    spans.push(Span::styled(&entry.git.subject, subject_style));
    spans.push(Span::raw(" "));
    spans.push(Span::styled(entry.git.author_and_date(), author_date_style));
    Spans::from(spans)
}

fn draw_list<B: Backend>(f: &mut Frame<B>, app: &mut App, chunk: tui::layout::Rect) {
    let height = chunk.height.saturating_sub(1); // top border

    app.log.set_view_height(height);
    let (pos, rows) = app.log.iter_view();
    let rows = rows
        .map(|entry| ListItem::new(log_line(entry, app)))
        .collect::<Vec<_>>();

    let list = List::new(rows)
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_type(BorderType::Plain)
                .title(app.title()),
        )
        .highlight_style(
            Style::default()
                .fg(tui::style::Color::Black)
                .bg(tui::style::Color::Green)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = ListState::default();
    state.select(Some(pos));
    f.render_stateful_widget(list, chunk, &mut state);
}

pub fn draw<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Min(5),
                Constraint::Length(1),
                Constraint::Length(1),
            ]
            .as_ref(),
        )
        .split(f.size());

    draw_list(f, app, chunks[0]);

    let status_style = Style::default().add_modifier(Modifier::REVERSED);
    let status_block = tui::widgets::Paragraph::new("status").style(status_style);
    f.render_widget(status_block, chunks[1]);

    match app.mode() {
        app::Mode::Normal => {
            let block = tui::widgets::Paragraph::new(app.status.as_str());
            f.render_widget(block, chunks[2]);
        }
        app::Mode::Command(_cmd) => {
            f.render_widget(app.textarea.widget(), chunks[2]);
        }
    }
}
