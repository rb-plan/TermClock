use std::time::Duration;
use crate::model::{ApiResponse, TemperatureData, TodoData};

// 温度传感器API调用
pub fn fetch_temperature_api(base_url: &str, device_code: &str) -> Option<String> {
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return None,
    };

    let request_body = serde_json::json!({
        "device_code": device_code,
        "page": {
            "num": 1,
            "size": 1
        }
    });

    let url = format!("{}/habitat/raw/list", base_url);
    match client.post(&url)
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .and_then(|r| r.error_for_status())
    {
        Ok(resp) => {
            match resp.json::<ApiResponse<TemperatureData>>() {
                Ok(api_resp) => {
                    if api_resp.code == 0 && !api_resp.data.rows.is_empty() {
                        let temp = api_resp.data.rows[0].values.temp;
                        Some(format!("{:.1}℃", temp))
                    } else {
                        None
                    }
                }
                Err(_) => None,
            }
        }
        Err(_) => None,
    }
}

// 待办事项API调用
pub fn fetch_todos_api(base_url: &str, limit: usize) -> Option<Vec<String>> {
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return None,
    };

    let request_body = serde_json::json!({
        "status": [0], // 0-代办 1-完成 2-草稿
        "page": {
            "num": 1,
            "size": limit
        }
    });

    let url = format!("{}/todo/list", base_url);
    match client.post(&url)
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .and_then(|r| r.error_for_status())
    {
        Ok(resp) => {
            match resp.json::<ApiResponse<TodoData>>() {
                Ok(api_resp) => {
                    if api_resp.code == 0 {
                        Some(api_resp.data.rows.into_iter().map(|row| {
                            format!("{} | {}", row.deadline, row.task)
                        }).collect())
                    } else {
                        None
                    }
                }
                Err(_) => None,
            }
        }
        Err(_) => None,
    }
}

// 从配置获取温度数据（优先API，回退到网络服务）
pub fn fetch_temperature_from_config(config: &crate::model::Config) -> Option<String> {
    // 优先使用API
    if let Some(base_url) = &config.api_base_url {
        if let Some(temp) = fetch_temperature_api(base_url, &config.device_code) {
            return Some(temp);
        }
    }
    
    // 检查配置文件中的API设置
    if let Some(file_cfg) = crate::config::load_yaml_config() {
        if let Some(base_url) = file_cfg.api_base_url {
            let device_code = file_cfg.device_code.unwrap_or_else(|| "SENS-FARM01".to_string());
            if let Some(temp) = fetch_temperature_api(&base_url, &device_code) {
                return Some(temp);
            }
        }
    }
    
    // 最后回退到网络服务
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

// 从配置获取待办事项数据（优先API，回退到文件）
pub fn load_todos_from_config(config: &crate::model::Config) -> Vec<String> {
    // Try YAML first
    if let Some(cfg) = crate::config::load_yaml_config() {
        // 优先使用API
        if let Some(base_url) = cfg.api_base_url.or_else(|| config.api_base_url.clone()) {
            let limit = cfg.todo_limit.or(config.todo_limit).unwrap_or(4);
            if let Some(list) = fetch_todos_api(&base_url, limit) { 
                return list; 
            }
        }
        
        // 如果指定了本地文件，加载文件
        if let Some(path) = cfg.todos_file {
            if let Ok(content) = std::fs::read_to_string(path) {
                return content
                    .lines()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect();
            }
        }
    }
    
    // 最后回退到默认文件
    const TODOS_FILE: &str = "todos.txt";
    match std::fs::read_to_string(TODOS_FILE) {
        Ok(content) => content
            .lines()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect(),
        Err(_) => Vec::new(),
    }
}
