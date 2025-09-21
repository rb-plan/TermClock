use serde::Deserialize;
use ratatui::style::Color;

// API响应数据结构
#[derive(Debug, Deserialize)]
pub struct ApiResponse<T> {
    pub code: i32,
    pub msg: String,
    pub data: T,
}

#[derive(Debug, Deserialize)]
pub struct TemperatureData {
    pub page: i32,
    pub page_size: i32,
    pub rows: Vec<TemperatureRow>,
    pub total: i32,
}

#[derive(Debug, Deserialize)]
pub struct TemperatureRow {
    pub created_at: String,
    pub device_code: String,
    pub id: i32,
    pub valid: bool,
    pub values: TemperatureValues,
}

#[derive(Debug, Deserialize)]
pub struct TemperatureValues {
    pub hum: f64,
    pub temp: f64,
}

#[derive(Debug, Deserialize)]
pub struct TodoData {
    pub page: i32,
    pub page_size: i32,
    pub rows: Vec<TodoRow>,
    pub total: i32,
}

#[derive(Debug, Deserialize)]
pub struct TodoRow {
    pub completed: bool,
    pub completed_time: Option<String>,
    pub create_time: String,
    pub deadline: String,
    pub id: i32,
    pub ipaddr: String,
    pub task: String,
    pub update_time: String,
}

// 配置文件结构
#[derive(Debug, Clone)]
pub struct FileConfig {
    pub api_base_url: Option<String>,
    pub device_code: Option<String>,
    pub temp_refresh_interval: Option<u64>,
    pub todo_ip_filter: Option<String>,
    pub todos_file: Option<String>,
    pub todo_task_max_chars: Option<usize>,
    pub todo_limit: Option<usize>,
    pub main_window_percent: u16,
}

// 应用配置结构
#[derive(Clone)]
pub struct Config {
    // scaling factors
    pub time_scale_x: u16,
    pub time_scale_y: u16,
    pub date_scale_x: u16,
    // colors
    pub time_color: Color,
    pub date_color: Color,
    pub todos_color: Color,
    // chime
    pub chime_enabled: bool,
    // api config
    pub api_base_url: Option<String>,
    pub device_code: String,
    // refresh intervals
    pub temp_refresh_interval: u64,
    // todo config
    pub todo_ip_filter: Option<String>,
    pub todo_limit: Option<usize>,
    pub main_window_percent: u16,
}

// 应用状态结构
pub struct App {
    pub last_temp_fetch: Option<std::time::Instant>,
    pub cached_temp: Option<String>,
    pub todos: Vec<String>,
    pub config: Config,
    pub last_chime_hour: Option<u32>,
    pub last_todos_refresh: Option<std::time::Instant>,
}
