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
    let get_bool = |key: &str| -> Option<bool> {
        map.get(&serde_yaml::Value::String(key.to_string()))
            .and_then(|v| v.as_bool())
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
        // UI配置
        time_scale_x: get_u16("time_scale_x"),
        time_scale_y: get_u16("time_scale_y"),
        date_scale_x: get_u16("date_scale_x"),
        time_color: get_string("time_color"),
        date_color: get_string("date_color"),
        todos_color: get_string("todos_color"),
        chime_enabled: get_bool("chime_enabled"),
    })
}

pub fn parse_args() -> Config {
    // 默认值
    let mut time_scale_x: u16 = 2;
    let mut time_scale_y: u16 = 2;
    let mut date_scale_x: u16 = 1;
    let mut main_window_percent: u16 = 70;

    let mut time_color = Color::White;
    let mut date_color = Color::Yellow;
    let mut todos_color = Color::White;
    let mut chime_enabled = true;
    let mut api_base_url: Option<String> = None;
    let mut device_code: String = "SENS-FARM01".to_string();
    let mut temp_refresh_interval: u64 = 5;
    let mut todo_ip_filter: Option<String> = None;

    // 从配置文件加载所有设置
    if let Some(file_cfg) = load_yaml_config() {
        // API配置
        if file_cfg.api_base_url.is_some() { api_base_url = file_cfg.api_base_url.clone(); }
        if let Some(device) = file_cfg.device_code { device_code = device; }
        if let Some(interval) = file_cfg.temp_refresh_interval { temp_refresh_interval = interval; }
        if file_cfg.todo_ip_filter.is_some() { todo_ip_filter = file_cfg.todo_ip_filter.clone(); }
        main_window_percent = file_cfg.main_window_percent;
        
        // UI配置
        if let Some(scale) = file_cfg.time_scale_x { time_scale_x = scale; }
        if let Some(scale) = file_cfg.time_scale_y { time_scale_y = scale; }
        if let Some(scale) = file_cfg.date_scale_x { date_scale_x = scale; }
        if let Some(chime) = file_cfg.chime_enabled { chime_enabled = chime; }
        
        // 颜色配置
        if let Some(color_str) = file_cfg.time_color {
            if let Some(color) = parse_color(&color_str) { time_color = color; }
        }
        if let Some(color_str) = file_cfg.date_color {
            if let Some(color) = parse_color(&color_str) { date_color = color; }
        }
        if let Some(color_str) = file_cfg.todos_color {
            if let Some(color) = parse_color(&color_str) { todos_color = color; }
        }
    }

    // 所有参数都从配置文件读取，不再支持命令行参数

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
