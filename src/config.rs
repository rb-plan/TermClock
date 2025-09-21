use std::fs;
use std::env;
use crate::model::{FileConfig, Config};
use ratatui::style::Color;

const DEFAULT_CONFIG_PATH: &str = "termclock.yml";

pub fn load_yaml_config() -> Option<FileConfig> {
    let path = env::var("TERMCLOCK_CONFIG").ok().unwrap_or_else(|| {
        if fs::metadata(DEFAULT_CONFIG_PATH).is_ok() {
            DEFAULT_CONFIG_PATH.to_string()
        } else {
            "termclock.yml".to_string()
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
    let get_u16 = |key: &str| -> Option<u16> {
        map.get(&serde_yaml::Value::String(key.to_string()))
            .and_then(|v| v.as_i64())
            .and_then(|n| if n > 0 { Some(n as u16) } else { None })
    };
    let get_u64 = |key: &str| -> Option<u64> {
        map.get(&serde_yaml::Value::String(key.to_string()))
            .and_then(|v| v.as_i64())
            .and_then(|n| if n > 0 { Some(n as u64) } else { None })
    };
    Some(FileConfig {
        api_base_url: get_string("api_base_url"),
        device_code: get_string("device_code"),
        temp_refresh_interval: get_u64("temp_refresh_interval"),
        todo_ip_filter: get_string("todo_ip_filter"),
        todos_file: get_string("todos_file"),
        todo_task_max_chars: get_usize("todo_task_max_chars"),
        todo_limit: get_usize("todo_limit"),
        main_window_percent: get_u16("main_window_percent").unwrap_or(80), 
    })
}

pub fn parse_args() -> Config {
    // defaults: date smaller than time
    let mut time_scale_x: u16 = 2;
    let mut time_scale_y: u16 = 2;
    let mut date_scale_x: u16 = 1;
    let mut main_window_percent: u16 = 70;

    let mut time_color = Color::White;
    let mut date_color = Color::Yellow;
    let mut todos_color = Color::White;
    let chime_enabled = true;
    let mut api_base_url: Option<String> = None;
    let mut device_code: String = "SENS-FARM01".to_string(); // 默认设备编号
    let mut temp_refresh_interval: u64 = 5; // 默认5秒
    let mut todo_ip_filter: Option<String> = None;

    // 1) Load YAML defaults if present
    if let Some(file_cfg) = load_yaml_config() {
        if file_cfg.api_base_url.is_some() { api_base_url = file_cfg.api_base_url.clone(); }
        if let Some(device) = file_cfg.device_code { device_code = device; }
        if let Some(interval) = file_cfg.temp_refresh_interval { temp_refresh_interval = interval; }
        if file_cfg.todo_ip_filter.is_some() { todo_ip_filter = file_cfg.todo_ip_filter.clone(); }
        // take main window split percent from file config
        main_window_percent = file_cfg.main_window_percent;
    }

    // 仅从命令行读取"字体/颜色"等展示相关参数；数据源与布局仅从配置文件读取
    let args: Vec<String> = env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--scale" => {
                if i + 1 < args.len() {
                    if let Ok(v) = args[i + 1].parse::<u16>() {
                        let v = v.max(1);
                        // 时间XY等比，日期横向略小一号
                        time_scale_x = v;
                        time_scale_y = v;
                        date_scale_x = v.saturating_sub(1).max(1);
                    }
                    i += 1;
                }
            }
            "--time-scale-x" => { if i + 1 < args.len() { if let Ok(v) = args[i+1].parse::<u16>() { time_scale_x = v.max(1); } i += 1; } }
            "--time-scale-y" => { if i + 1 < args.len() { if let Ok(v) = args[i+1].parse::<u16>() { time_scale_y = v.max(1); } i += 1; } }
            "--date-scale-x" => { if i + 1 < args.len() { if let Ok(v) = args[i+1].parse::<u16>() { date_scale_x = v.max(1); } i += 1; } }
            "--time-color" => {
                if i + 1 < args.len() {
                    if let Some(c) = parse_color(&args[i + 1]) { time_color = c; }
                    i += 1;
                }
            }
            "--date-color" => {
                if i + 1 < args.len() {
                    if let Some(c) = parse_color(&args[i + 1]) { date_color = c; }
                    i += 1;
                }
            }
            "--todos-color" => {
                if i + 1 < args.len() {
                    if let Some(c) = parse_color(&args[i + 1]) { todos_color = c; }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    Config { 
        time_scale_x, 
        time_scale_y, 
        date_scale_x, 
        time_color, 
        date_color, 
        todos_color, 
        chime_enabled, 
        api_base_url, 
        device_code,
        temp_refresh_interval,
        todo_ip_filter, 
        todo_limit: None, 
        main_window_percent 
    }
}

#[allow(dead_code)]
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
