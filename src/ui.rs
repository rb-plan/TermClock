use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph},
    Frame,
};
use chrono::Datelike;
use crate::model::{App, Config};

// 绘制时钟
pub fn draw_clock(f: &mut Frame, area: Rect, config: &Config) {
    let now = chrono::Local::now();
    let time_str = now.format("%H:%M:%S").to_string();
    let lines = render_big_time(&time_str, config.time_scale_x, config.time_scale_y);

    let mut text: Vec<Line> = lines
        .into_iter()
        .map(|s| Line::from(Span::styled(s, Style::default().fg(config.time_color).add_modifier(Modifier::BOLD))))
        .collect();
    
    // Append centered date line right under time using smallest characters
    let gap_lines = ((config.time_scale_y as usize) + 1) / 2;
    for _ in 0..gap_lines {
        text.push(Line::from(""));
    }
    let date_small = format_date_cn();
    text.push(Line::from(Span::styled(
        date_small,
        Style::default().fg(config.date_color),
    )));
    
    // Vertical centering within the given area by pre-padding empty lines
    let content_lines = text.len();
    let area_height = area.height as usize;
    let pad_top = if area_height > content_lines {
        (area_height - content_lines) / 2
    } else {
        0
    };
    let mut centered: Vec<Line> = Vec::with_capacity(pad_top + content_lines);
    for _ in 0..pad_top {
        centered.push(Line::from(""));
    }
    centered.extend(text);

    let para = Paragraph::new(centered).alignment(ratatui::layout::Alignment::Center);
    f.render_widget(para, area);
}

// 绘制侧边栏（温度和待办事项）
pub fn draw_sidebar(
    f: &mut Frame,
    area: Rect,
    app: &mut App,
) {
    // 横向拆分：左=温度计+Todo，右=二维码
    let hchunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(100), Constraint::Percentage(0)])
        .split(area);
    let left = hchunks[0];
    
    // 左列：原有垂直布局
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),  // temperature
            Constraint::Min(1),     // todos
        ])
        .split(left);

    let temp_str = app.temperature();
    let parsed = parse_temp_celsius(&temp_str);
    draw_temperature_widget(f, chunks[0], parsed);
    draw_todos_widget(f, chunks[1], app);
}

// 绘制温度组件
fn draw_temperature_widget(
    f: &mut Frame,
    area: Rect,
    parsed_temp: Option<i32>,
) {
    // Dual-line thermometer centered to 80% width: top labels, mid ticks, bottom bar
    let width = area.width as usize;
    let mut usable = ((width as f64) * 0.9).round() as usize;
    if usable > width { usable = width; }
    if usable < 30 { usable = 30.min(width); }
    let pad = width.saturating_sub(usable) / 2;
    let min_c = -10.0f64;
    let max_c = 50.0f64;
    let pos = parsed_temp.map(|v| ((v as f64 - min_c) / (max_c - min_c)).clamp(0.0,1.0)).unwrap_or(0.0);
    let bar_len = (pos * (usable as f64)).round() as usize;

    let mut tick_chars: Vec<char> = vec!['─'; usable];
    let tick_degs = [-10, 0, 10, 20, 30, 40, 50];
    let mut tick_positions: Vec<usize> = Vec::with_capacity(tick_degs.len());
    for &deg in &tick_degs {
        let t = (deg as f64 - min_c) / (max_c - min_c);
        let idx = (t * usable as f64).round() as usize;
        if idx < usable { tick_chars[idx] = '┴'; tick_positions.push(idx); }
    }
    let mut label_chars: Vec<char> = vec![' '; usable];
    for (&deg, &idx) in tick_degs.iter().zip(tick_positions.iter()) {
        let s = format!("{}", deg);
        let start = idx.saturating_sub(s.len()/2).min(usable.saturating_sub(s.len()));
        for (i, ch) in s.chars().enumerate() {
            if start + i < usable { label_chars[start + i] = ch; }
        }
    }
    let pad_str = " ".repeat(pad);
    let labels_line = Line::from(vec![
        Span::raw(pad_str.clone()),
        Span::styled(label_chars.into_iter().collect::<String>(), Style::default().fg(Color::LightRed)),
    ]);
    let ticks_line = Line::from(vec![
        Span::raw(pad_str.clone()),
        Span::styled(tick_chars.into_iter().collect::<String>(), Style::default().fg(Color::LightRed)),
    ]);

    let mut bottom_chars: Vec<char> = vec![' '; usable];
    for i in 0..usable { if i < bar_len { bottom_chars[i] = '━'; } }
    let label = parsed_temp.map(|v| format!(" {v}℃")).unwrap_or_else(|| " --".to_string());
    let overlay_at = bar_len.min(usable.saturating_sub(label.len()));
    for (i, ch) in label.chars().enumerate() { if overlay_at + i < usable { bottom_chars[overlay_at + i] = ch; } }
    let bottom_line = Line::from(vec![
        Span::raw(pad_str.clone()),
        Span::styled(bottom_chars.into_iter().collect::<String>(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
    ]);

    let temp_widget = Paragraph::new(vec![labels_line, ticks_line, bottom_line]).alignment(ratatui::layout::Alignment::Left);
    f.render_widget(temp_widget, area);
}

// 绘制待办事项组件
fn draw_todos_widget(
    f: &mut Frame,
    area: Rect,
    app: &App,
) {
    // Todo 居中 80% 且区域内左对齐
    let width = area.width as usize;
    let mut usable = ((width as f64) * 0.8).round() as usize;
    if usable > width { usable = width; }
    let pad = width.saturating_sub(usable) / 2;
    let pad_str = " ".repeat(pad);

    let mut max_chars: Option<usize> = None;
    if let Some(cfg) = crate::config::load_yaml_config() { 
        max_chars = cfg.todo_task_max_chars; 
    }
    let truncate = |s: &str| -> String {
        if let Some(m) = max_chars { 
            if s.chars().count() > m { 
                let mut c = s.chars(); 
                return c.by_ref().take(m).collect::<String>() + "…"; 
            } 
        }
        s.to_string()
    };

    let items: Vec<ListItem> = if app.todos.is_empty() {
        vec![ListItem::new(Span::raw(format!("{}(no todos)", pad_str)))]
    } else {
        app.todos
            .iter()
            .map(|t| {
                let content = truncate(t);
                ListItem::new(Span::styled(format!("{}{}", pad_str, content), Style::default().fg(app.config.todos_color)))
            })
            .collect()
    };
    let todos_widget = List::new(items);
    f.render_widget(todos_widget, area);
}

// 解析温度值
fn parse_temp_celsius(s: &str) -> Option<i32> {
    // Accept formats like "29℃", "29°C", "29", "24.5℃", etc.
    let trimmed = s.trim().trim_end_matches('C').trim_end_matches('°').trim_end_matches('℃').trim();
    // 先尝试解析为f64，然后转换为i32
    if let Ok(temp_f) = trimmed.parse::<f64>() {
        Some(temp_f.round() as i32)
    } else {
        trimmed.parse::<i32>().ok()
    }
}

// 渲染大字体时间
fn render_big_time(time: &str, scale_x: u16, scale_y: u16) -> Vec<String> {
    // 7-row big digits using a simple ASCII font
    const FONT: [[&str; 7]; 12] = [
        // 0
        [
            "  ███  ",
            " █   █ ",
            " █  ██ ",
            " █ █ █ ",
            " ██  █ ",
            " █   █ ",
            "  ███  ",
        ],
        // 1
        [
            "   █   ",
            "  ██   ",
            "   █   ",
            "   █   ",
            "   █   ",
            "   █   ",
            "  ███  ",
        ],
        // 2
        [
            "  ███  ",
            " █   █ ",
            "     █ ",
            "   ██  ",
            "  █    ",
            " █     ",
            " █████ ",
        ],
        // 3
        [
            " █████ ",
            "     █ ",
            "    ██ ",
            "   ███ ",
            "     █ ",
            " █   █ ",
            "  ███  ",
        ],
        // 4
        [
            "    ██ ",
            "   █ █ ",
            "  █  █ ",
            " █   █ ",
            " ██████",
            "     █ ",
            "     █ ",
        ],
        // 5
        [
            " █████ ",
            " █     ",
            " ████  ",
            "     █ ",
            "     █ ",
            " █   █ ",
            "  ███  ",
        ],
        // 6
        [
            "  ███  ",
            " █     ",
            " █     ",
            " ████  ",
            " █   █ ",
            " █   █ ",
            "  ███  ",
        ],
        // 7
        [
            " █████ ",
            "     █ ",
            "    █  ",
            "   █   ",
            "  █    ",
            "  █    ",
            "  █    ",
        ],
        // 8
        [
            "  ███  ",
            " █   █ ",
            " █   █ ",
            "  ███  ",
            " █   █ ",
            " █   █ ",
            "  ███  ",
        ],
        // 9
        [
            "  ███  ",
            " █   █ ",
            " █   █ ",
            "  ████ ",
            "     █ ",
            "     █ ",
            "  ███  ",
        ],
        // ':'
        [
            "       ",
            "   ░   ",
            "       ",
            "       ",
            "       ",
            "   ░   ",
            "       ",
        ],
        // ' '
        [
            "       ",
            "       ",
            "       ",
            "       ",
            "       ",
            "       ",
            "       ",
        ],
    ];

    let mut base_rows = vec![String::new(); 7];
    for ch in time.chars() {
        let idx = match ch {
            '0' => 0,
            '1' => 1,
            '2' => 2,
            '3' => 3,
            '4' => 4,
            '5' => 5,
            '6' => 6,
            '7' => 7,
            '8' => 8,
            '9' => 9,
            ':' => 10,
            _ => 11,
        };
        for (r, line) in FONT[idx].iter().enumerate() {
            if !base_rows[r].is_empty() {
                base_rows[r].push_str("  ");
            }
            base_rows[r].push_str(line);
        }
    }
    // scale horizontally and vertically with independent factors
    let sx = scale_x.max(1) as usize;
    let sy = scale_y.max(1) as usize;
    let mut scaled_rows: Vec<String> = Vec::new();
    for row in base_rows {
        // Horizontal scaling
        let mut hscaled = String::with_capacity(row.len() * sx);
        for ch in row.chars() {
            for _ in 0..sx {
                hscaled.push(ch);
            }
        }
        // Vertical scaling
        for _ in 0..sy {
            scaled_rows.push(hscaled.clone());
        }
    }
    scaled_rows
}

// 格式化中文日期
fn format_date_cn() -> String {
    let now = chrono::Local::now();
    let weekday = match now.weekday().number_from_monday() {
        1 => "星期一",
        2 => "星期二",
        3 => "星期三",
        4 => "星期四",
        5 => "星期五",
        6 => "星期六",
        _ => "星期日",
    };
    // mm/dd/yyyy 星期X
    format!("{}/{}/{} {}",
        now.format("%m").to_string(),
        now.format("%d").to_string(),
        now.format("%Y").to_string(),
        weekday)
}
