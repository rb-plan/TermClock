#[derive(Debug, Clone)]
struct FileConfig {
    mysql_url: Option<String>,
    todo_db_url: Option<String>,
    todo_ip_filter: Option<String>,
    todos_file: Option<String>,
    todo_task_max_chars: Option<usize>,
    todo_limit: Option<usize>,
}

fn load_yaml_config() -> Option<FileConfig> {
    let path = env::var("TERMCLOCK_CONFIG").ok().unwrap_or_else(|| {
        if fs::metadata(DEFAULT_CONFIG_PATH).is_ok() {
            DEFAULT_CONFIG_PATH.to_string()
        } else {
            "conf.yaml".to_string()
        }
    });
    let content = fs::read_to_string(path).ok()?;
    // Parse via generic Value to avoid serde_derive runtime
    let value: serde_yaml::Value = serde_yaml::from_str(&content).ok()?;
    let map = value.as_mapping()?;
    let get_string = |key: &str| -> Option<String> {
        map.get(&serde_yaml::Value::String(key.to_string()))
            .and_then(|v| v.as_str().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
    };
    let get_usize = |key: &str| -> Option<usize> {
        map.get(&serde_yaml::Value::String(key.to_string()))
            .and_then(|v| v.as_i64())
            .and_then(|n| if n > 0 { Some(n as usize) } else { None })
    };
    Some(FileConfig {
        mysql_url: get_string("mysql_url"),
        todo_db_url: get_string("todo_db_url"),
        todo_ip_filter: get_string("todo_ip_filter"),
        todos_file: get_string("todos_file"),
        todo_task_max_chars: get_usize("todo_task_max_chars"),
        todo_limit: get_usize("todo_limit"),
    })
}
use std::fs;
use std::io;
use std::io::Write;
use std::time::{Duration, Instant};
use std::env;

use chrono::{Datelike, Local, Timelike};
use chrono::TimeZone;
use mysql::{prelude::Queryable, Opts, Pool};
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, Paragraph};
use ratatui::Terminal;

const TEMP_FETCH_INTERVAL: Duration = Duration::from_secs(10);
const REFRESH_INTERVAL: Duration = Duration::from_millis(200);
const TODOS_FILE: &str = "todos.txt";
const DEFAULT_CONFIG_PATH: &str = "termclock.yml";

struct App {
    last_temp_fetch: Option<Instant>,
    cached_temp: Option<String>,
    todos: Vec<String>,
    config: Config,
    last_chime_hour: Option<u32>,
    last_todos_refresh: Option<Instant>,
}

impl App {
    fn new(config: Config) -> Self {
        Self {
            last_temp_fetch: None,
            cached_temp: None,
            todos: load_todos_from_config(&config),
            config,
            last_chime_hour: None,
            last_todos_refresh: None,
        }
    }

    fn temperature(&mut self) -> String {
        let now = Instant::now();
        let needs_fetch = match self.last_temp_fetch {
            None => true,
            Some(ts) => now.duration_since(ts) >= TEMP_FETCH_INTERVAL,
        };
        if needs_fetch {
            if let Some(temp) = fetch_temperature_from_config(&self.config) {
                self.cached_temp = Some(temp);
                self.last_temp_fetch = Some(now);
            } else {
                self.cached_temp = Some("--".to_string());
                self.last_temp_fetch = Some(now);
            }
        }
        self.cached_temp.clone().unwrap_or_else(|| "--".into())
    }
}
fn parse_temp_celsius(s: &str) -> Option<i32> {
    // Accept formats like "29℃", "29°C", "29", " +5 ", etc.
    let trimmed = s.trim().trim_end_matches('C').trim_end_matches('°').trim_end_matches('℃').trim();
    trimmed.parse::<i32>().ok()
}

fn main() -> io::Result<()> {
    let config = parse_args();
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(config);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| {
            let size = f.size();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(size);

            draw_clock(f, chunks[0], &app.config);
            draw_sidebar(f, chunks[1], &mut app);
        })?;

        // Hourly chime: on the hour at second 0, once per hour
        if app.config.chime_enabled {
            let now = Local::now();
            if now.minute() == 0 && now.second() == 0 {
                let hour = now.hour();
                if app.last_chime_hour != Some(hour) {
                    chime_hour(hour);
                    app.last_chime_hour = Some(hour);
                }
            }
        }

        // Periodically refresh todos (every 5 seconds)
        let now_instant = Instant::now();
        let need_todos_refresh = match app.last_todos_refresh {
            None => true,
            Some(ts) => now_instant.duration_since(ts) >= Duration::from_secs(5),
        };
        if need_todos_refresh {
            app.todos = load_todos_from_config(&app.config);
            app.last_todos_refresh = Some(now_instant);
        }

        let timeout = REFRESH_INTERVAL
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    use crossterm::event::KeyModifiers;
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Esc => break,
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                        KeyCode::Char('r') => {
                            // Reload todos and temp on demand
                            app.todos = load_todos_from_config(&app.config);
                            app.last_temp_fetch = None;
                        }
                        _ => {}
                    }
                }
            }
        }

        if last_tick.elapsed() >= REFRESH_INTERVAL {
            last_tick = Instant::now();
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn draw_clock(f: &mut ratatui::Frame<'_>, area: Rect, config: &Config) {
    let now = Local::now();
    let time_str = now.format("%H:%M:%S").to_string();
    let lines = render_big_time(&time_str, config.time_scale_x, config.time_scale_y);

    let mut text: Vec<Line> = lines
        .into_iter()
        .map(|s| Line::from(Span::styled(s, Style::default().fg(config.time_color).add_modifier(Modifier::BOLD))))
        .collect();
    // Append centered date line right under time using smallest characters
    // Gap scales with font size for better visual balance (based on Y scale of time)
    let gap_lines = ((config.time_scale_y as usize) + 1) / 2; // 1 for 1-2, 2 for 3-4, etc.
    for _ in 0..gap_lines {
        text.push(Line::from(""));
    }
    let date_small = format_date_cn();
    // ensure the field remains used; date renders at minimal size regardless of scale
    let _ = config.date_scale_x;
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
// removed: unused

fn draw_sidebar(
    f: &mut ratatui::Frame<'_>,
    area: Rect,
    app: &mut App,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // temperature
            Constraint::Min(1),     // todos
        ])
        .split(area);

    let temp_str = app.temperature();
    let parsed = parse_temp_celsius(&temp_str);
    // Dual-line thermometer centered to 80% width: top line shows ticks and labels (LightBlue), bottom shows current bar (Yellow) with value
    let area = chunks[0];
    let width = area.width as usize;
    let mut usable = ((width as f64) * 0.8).round() as usize;
    if usable > width { usable = width; }
    if usable < 30 { usable = 30.min(width); }
    let pad = width.saturating_sub(usable) / 2;
    let min_c = -10.0f64;
    let max_c = 50.0f64;
    let pos = parsed.map(|v| ((v as f64 - min_c) / (max_c - min_c)).clamp(0.0,1.0)).unwrap_or(0.0);
    let bar_len = (pos * (usable as f64)).round() as usize;

    // Top line: baseline + ticks; labels will be on a separate line above
    let mut tick_chars: Vec<char> = vec!['─'; usable];
    let tick_degs = [-10, 0, 10, 20, 30, 40, 50];
    let mut tick_positions: Vec<usize> = Vec::with_capacity(tick_degs.len());
    for &deg in &tick_degs {
        let t = (deg as f64 - min_c) / (max_c - min_c);
        let idx = (t * usable as f64).round() as usize;
        if idx < usable { tick_chars[idx] = '┬'; tick_positions.push(idx); }
    }
    // Labels line (above the tick line)
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

    // Bottom line: bar in Yellow up to current position, rest spaces; overlay value near bar end
    let mut bottom_chars: Vec<char> = vec![' '; usable];
    for i in 0..usable { if i < bar_len { bottom_chars[i] = '━'; } }
    let label = parsed.map(|v| format!(" {v}℃")).unwrap_or_else(|| " --".to_string());
    let overlay_at = bar_len.min(usable.saturating_sub(label.len()));
    for (i, ch) in label.chars().enumerate() { if overlay_at + i < usable { bottom_chars[overlay_at + i] = ch; } }
    let bottom_line = Line::from(vec![
        Span::raw(pad_str),
        Span::styled(bottom_chars.into_iter().collect::<String>(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
    ]);

    let temp_widget = Paragraph::new(vec![labels_line, ticks_line, bottom_line]).alignment(ratatui::layout::Alignment::Left);
    f.render_widget(temp_widget, area);

    // Apply max chars from config file if provided
    let mut max_chars: Option<usize> = None;
    if let Some(cfg) = load_yaml_config() { max_chars = cfg.todo_task_max_chars; }
    let truncate = |s: &str| -> String {
        if let Some(m) = max_chars { if s.chars().count() > m { let mut c = s.chars(); return c.by_ref().take(m).collect::<String>() + "…"; } }
        s.to_string()
    };

    // Center to 80% width and left-align content inside that region
    let todos_area = chunks[1];
    let width = todos_area.width as usize;
    let mut usable = ((width as f64) * 0.8).round() as usize;
    if usable > width { usable = width; }
    let pad = width.saturating_sub(usable) / 2;
    let pad_str = " ".repeat(pad);

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
    f.render_widget(todos_widget, todos_area);
}

fn load_todos_from_config(config: &Config) -> Vec<String> {
    // Try YAML first
    if let Some(cfg) = load_yaml_config() {
        // If todo db configured, load from DB
        if let Some(db_url) = cfg.todo_db_url.or_else(|| config.todo_db_url.clone()) {
            let ip = cfg.todo_ip_filter.or_else(|| config.todo_ip_filter.clone());
            let limit = cfg.todo_limit.or(config.todo_limit).unwrap_or(4);
            if let Some(list) = fetch_todos_mysql(&db_url, ip, limit) { return list; }
        }
        // else, if a local file specified in YAML, load file
        if let Some(path) = cfg.todos_file {
            if let Ok(content) = fs::read_to_string(path) {
                return content
                    .lines()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect();
            }
        }
    }
    // fallback to default file
    match fs::read_to_string(TODOS_FILE) {
        Ok(content) => content
            .lines()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn fetch_temperature_from_config(config: &Config) -> Option<String> {
    if let Some(url) = &config.mysql_url {
        return fetch_temperature_mysql(url);
    }
    if let Some(file_cfg) = load_yaml_config() {
        if let Some(url) = file_cfg.mysql_url {
            return fetch_temperature_mysql(&url);
        }
    }
    // fallback to network service if no mysql configured
    let url = "https://wttr.in/?format=%t";
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return None,
    };
    match client.get(url).send().and_then(|r| r.error_for_status()) {
        Ok(resp) => match resp.text() {
            Ok(text) => Some(text.trim().replace("°C", "℃")),
            Err(_) => None,
        },
        Err(_) => None,
    }
}

fn fetch_temperature_mysql(url: &str) -> Option<String> {
    // Expect url like: mysql://user:pass@host:port/db
    let opts = Opts::from_url(url).ok()?;
    let pool = Pool::new(opts).ok()?;
    let mut conn = pool.get_conn().ok()?;

    // Query last 5 minutes latest temp from t_sensors
    // Using ctime within last 5 minutes, take the newest row
    let query = r#"
        SELECT temp
        FROM t_sensors
        WHERE ctime >= (NOW() - INTERVAL 5 MINUTE)
        ORDER BY ctime DESC
        LIMIT 1
    "#;
    let row: Option<i8> = conn
        .exec_first(query, ())
        .ok()
        .flatten();

    row.map(|t| format!("{}℃", t))
}

fn fetch_todos_mysql(url: &str, _ip_filter: Option<String>, limit: usize) -> Option<Vec<String>> {
    let opts = Opts::from_url(url).ok()?;
    let pool = Pool::new(opts).ok()?;
    let mut conn = pool.get_conn().ok()?;

    let sql = format!(
        "SELECT LEFT(task, 80), update_time FROM todo WHERE completed = 0 ORDER BY update_time DESC LIMIT {}",
        limit.max(1)
    );
    let rows: Option<Vec<(String, i64)>> = conn.exec(sql, ()).ok();
    rows.map(|v| v.into_iter().map(|(task, utime)| format!("{} | {}", format_deadline(utime), task)).collect())
}

fn format_deadline(epoch_ms: i64) -> String {
    if epoch_ms <= 0 { return "--".to_string(); }
    // detect if value is seconds or ms
    let epoch_s = if epoch_ms > 10_000_000_000 { epoch_ms / 1000 } else { epoch_ms };
    let dt = chrono::Local.timestamp_opt(epoch_s, 0).single();
    match dt {
        Some(t) => t.format("%Y-%m-%d %H:%M:%S").to_string(),
        None => "--".to_string(),
    }
}

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

// ---------------- Config & CLI ----------------
#[derive(Clone)]
struct Config {
    // scaling factors
    time_scale_x: u16,
    time_scale_y: u16,
    date_scale_x: u16,
    // colors
    time_color: Color,
    date_color: Color,
    todos_color: Color,
    // chime
    chime_enabled: bool,
    // mysql
    mysql_url: Option<String>,
    // todo db config (optional, falls back to mysql_url if not provided)
    todo_db_url: Option<String>,
    todo_ip_filter: Option<String>,
    todo_limit: Option<usize>,
}

fn parse_args() -> Config {
    // defaults: date smaller than time
    let mut time_scale_x: u16 = 2;
    let mut time_scale_y: u16 = 2;
    let mut date_scale_x: u16 = 1;

    let mut time_color = Color::White;
    let mut date_color = Color::Yellow;
    let mut todos_color = Color::White;
    let mut chime_enabled = true;
    let mut mysql_url: Option<String> = None;
    let mut todo_db_url: Option<String> = None;
    let mut todo_ip_filter: Option<String> = None;

    // 1) Load YAML defaults if present
    if let Some(file_cfg) = load_yaml_config() {
        if file_cfg.mysql_url.is_some() { mysql_url = file_cfg.mysql_url.clone(); }
        if file_cfg.todo_db_url.is_some() { todo_db_url = file_cfg.todo_db_url.clone(); }
        if file_cfg.todo_ip_filter.is_some() { todo_ip_filter = file_cfg.todo_ip_filter.clone(); }
    }

    let args: Vec<String> = env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--scale" => {
                if i + 1 < args.len() {
                    if let Ok(v) = args[i + 1].parse::<u16>() {
                        let v = v.max(1);
                        time_scale_x = v;
                        time_scale_y = v;
                        date_scale_x = v.saturating_sub(1).max(1); // date slightly smaller
                    }
                    i += 1;
                }
            }
            "--time-scale-x" => { if i + 1 < args.len() { if let Ok(v) = args[i+1].parse::<u16>() { time_scale_x = v.max(1);} i += 1; } }
            "--time-scale-y" => { if i + 1 < args.len() { if let Ok(v) = args[i+1].parse::<u16>() { time_scale_y = v.max(1);} i += 1; } }
            "--date-scale-x" => { if i + 1 < args.len() { if let Ok(v) = args[i+1].parse::<u16>() { date_scale_x = v.max(1);} i += 1; } }
            "--time-color" => {
                if i + 1 < args.len() {
                    if let Some(c) = parse_color(&args[i + 1]) {
                        time_color = c;
                    }
                    i += 1;
                }
            }
            "--date-color" => {
                if i + 1 < args.len() {
                    if let Some(c) = parse_color(&args[i + 1]) {
                        date_color = c;
                    }
                    i += 1;
                }
            }
            "--todos-color" => {
                if i + 1 < args.len() {
                    if let Some(c) = parse_color(&args[i + 1]) {
                        todos_color = c;
                    }
                    i += 1;
                }
            }
            "--no-chime" => { chime_enabled = false; }
            "--mysql-url" => {
                if i + 1 < args.len() {
                    mysql_url = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--todo-db-url" => {
                if i + 1 < args.len() {
                    todo_db_url = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            "--todo-ip" => {
                if i + 1 < args.len() {
                    todo_ip_filter = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    Config { time_scale_x, time_scale_y, date_scale_x, time_color, date_color, todos_color, chime_enabled, mysql_url, todo_db_url, todo_ip_filter, todo_limit: None }
}

fn parse_color(name: &str) -> Option<Color> {
    match name.to_ascii_lowercase().as_str() {
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" | "yello" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "white" => Some(Color::White),
        "gray" | "grey" => Some(Color::Gray),
        "darkgray" | "darkgrey" => Some(Color::DarkGray),
        "lightred" => Some(Color::LightRed),
        "lightgreen" => Some(Color::LightGreen),
        "lightyellow" => Some(Color::LightYellow),
        "lightblue" => Some(Color::LightBlue),
        "lightmagenta" => Some(Color::LightMagenta),
        "lightcyan" => Some(Color::LightCyan),
        "orange" => Some(Color::Rgb(255, 165, 0)),
        _ => None,
    }
}

fn chime_hour(hour24: u32) {
    // Normal hour: 1 long beep (~1s). At 12 o'clock: 2 long beeps.
    let count = if hour24 == 12 { 2 } else { 1 };
    for i in 0..count {
        beep_long(Duration::from_millis(1000));
        if i + 1 < count { std::thread::sleep(Duration::from_millis(200)); }
    }
}

fn beep_long(duration: Duration) {
    // Emit BEL repeatedly to approximate a long beep; terminal decides the sound.
    // If the terminal does not beep, no sound may be produced.
    let mut out = io::stdout();
    let step = Duration::from_millis(50);
    let mut elapsed = Duration::from_millis(0);
    while elapsed < duration {
        let _ = write!(out, "\x07");
        let _ = out.flush();
        std::thread::sleep(step);
        elapsed += step;
    }
}

fn format_date_cn() -> String {
    let now = Local::now();
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
