mod model;
mod api;
mod ui;
mod config;

use std::io;
use std::time::{Duration, Instant};
use std::io::Write;

use chrono::{Local, Timelike};
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::Terminal;

use model::{App, Config};
use config::parse_args;
use api::{fetch_temperature_from_config, load_todos_from_config};

const REFRESH_INTERVAL: Duration = Duration::from_millis(200);

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
        let temp_fetch_interval = Duration::from_secs(self.config.temp_refresh_interval);
        let needs_fetch = match self.last_temp_fetch {
            None => true,
            Some(ts) => now.duration_since(ts) >= temp_fetch_interval,
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
                .constraints([Constraint::Percentage(app.config.main_window_percent), Constraint::Percentage(100 - app.config.main_window_percent)])
                .split(size);

            ui::draw_clock(f, chunks[0], &app.config);
            ui::draw_sidebar(f, chunks[1], &mut app);
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