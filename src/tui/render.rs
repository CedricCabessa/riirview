use crate::models::{Notification, NotificationState, NotificationType};
use crate::tui::App;
use crate::tui::messages::InputMode;
use chrono_humanize::{Accuracy, HumanTime, Tense};
use ratatui::Frame;
use ratatui::style::{Modifier, Style};
use ratatui::text::*;
use ratatui::widgets::{ListState, Paragraph};
use ratatui::{
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::Color,
    text::Line,
    widgets::{Block, Clear, List},
};
use tui_textarea::TextArea;

pub(crate) fn draw_frame(
    app: &App,
    frame: &mut Frame,
    notifications: &[Notification],
    list_state: &mut ListState,
    search_text: &TextArea,
    status: Result<String, String>,
) {
    let status = match status {
        Ok(msg) => Span::raw(msg),
        Err(err) => Span::styled(err, Style::default().fg(Color::Red)),
    };

    let head = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]);
    let [status_rect, help_rect] = head.areas(frame.area());
    frame.render_widget(Line::from(status).alignment(Alignment::Left), status_rect);
    frame.render_widget(
        Line::from("? for Help").alignment(Alignment::Right),
        help_rect,
    );

    let main_area = if app.input_mode == InputMode::Search {
        let layout_v = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .spacing(1);
        let [_, input_area, main_area] = layout_v.areas(frame.area());
        let layout_input = Layout::horizontal([Constraint::Length(4), Constraint::Fill(1)]);
        let [icon_area, search_area] = layout_input.areas(input_area);
        frame.render_widget(Line::from("🐕>"), icon_area);
        frame.render_widget(search_text, search_area);
        main_area
    } else {
        let layout_v = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).spacing(1);
        let [_, main_area] = layout_v.areas(frame.area());
        main_area
    };

    let list = List::new(notifications).highlight_style(Modifier::REVERSED);
    frame.render_stateful_widget(list, main_area, list_state);
    if let Some(popup) = &app.popup {
        let area = frame.area();
        let lines: Vec<Line> = popup.content.split('\n').map(Line::from).collect();
        let max_len = lines
            .iter()
            .max_by(|a, b| a.width().cmp(&b.width()))
            .expect("popup content always has at least one line");
        let height: u16 = (lines.len() + 3).try_into().unwrap_or(u16::MAX);
        let width: u16 = (max_len.width() + 3).try_into().unwrap_or(u16::MAX);
        let area = popup_area(area, height, width);
        let paragraph = Paragraph::new(lines);
        let block = Block::bordered().title(popup.title.as_str());
        frame.render_widget(Clear, area); //this clears out the background
        frame.render_widget(paragraph.block(block), area);
    }
}

fn popup_area(area: Rect, lines: u16, columns: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(lines)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Length(columns)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}

impl From<&Notification> for Text<'_> {
    fn from(notification: &Notification) -> Self {
        let icon = match notification.type_ {
            NotificationType::Issue => match notification.state {
                NotificationState::Open => "🐛",
                NotificationState::Resolved => "🦋",
                NotificationState::Canceled => "🪳",
                NotificationState::Draft => unreachable!(),
            },
            NotificationType::Release => "🚢",
            NotificationType::PullRequest => match notification.state {
                NotificationState::Open => "📬",
                NotificationState::Resolved => "📪",
                NotificationState::Canceled => "❌",
                NotificationState::Draft => "📝",
            },
        };
        let time = HumanTime::from(notification.updated_at.and_utc())
            .to_text_en(Accuracy::Rough, Tense::Past);
        let txt = format!(
            "{score:>3} {icon} {time:<15} {author:15} {repo:<30} {title}",
            score = notification.score + notification.score_boost,
            icon = icon,
            time = ellipsis(&time, 15),
            author = ellipsis(&notification.author, 15),
            repo = ellipsis(&notification.repo, 30),
            title = ellipsis(&notification.title, 80),
        );

        let style = Style::default();
        let style = if notification.unread {
            style.add_modifier(Modifier::BOLD)
        } else {
            style
        };

        Text::styled(txt, style)
    }
}

pub(crate) fn ellipsis(txt: &str, max_len: usize) -> String {
    let char_count = txt.chars().count();
    if char_count > max_len {
        let end = txt
            .char_indices()
            .nth(max_len - 1)
            .map(|(i, _)| i)
            .unwrap_or(txt.len());
        let mut res = txt[..end].to_owned();
        res.push('…');
        res
    } else {
        let mut res = txt.to_owned();
        let padding = " ".repeat(max_len - char_count);
        res.push_str(&padding);
        res
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ellipsis() {
        assert_eq!(ellipsis("lorem ipsum", 5), "lore…");
        assert_eq!(ellipsis("lorem ipsum", 13), "lorem ipsum  ");
    }

    #[test]
    fn test_ellipsis_multibyte_utf8() {
        // Multi-byte chars: 'é' is 2 bytes, '日' is 3 bytes
        // This should not panic on multi-byte character boundaries
        assert_eq!(ellipsis("héllo", 4), "hél…");
        assert_eq!(ellipsis("日本語テスト", 4), "日本語…");
    }

    #[test]
    fn test_ellipsis_multibyte_padding() {
        // Padding calculation should use char count, not byte length
        assert_eq!(ellipsis("hé", 5), "hé   ");
    }
}
