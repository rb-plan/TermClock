# # TermClock

一个基于Rust的终端时钟应用，支持温度显示和待办事项列表。

## 功能特性

- 大字体时间显示
- 温度传感器数据（通过API获取）
- 待办事项列表（通过API获取）
- 可配置的界面布局和颜色
- 模块化代码结构

## 代码结构

```
src/
├── main.rs      # 主程序入口和事件循环
├── model.rs     # 数据结构和模型定义
├── api.rs       # API调用相关功能
├── ui.rs        # UI绘制和渲染
└── config.rs    # 配置解析和管理
```

## API支持

应用现在支持通过API接口获取数据：

### 温度传感器API
- 端点：`/habitat/raw/list`
- 方法：POST
- 请求体：
```json
{
    "device_code": "SENS-FARM01",
    "page": {
        "num": 1,
        "size": 1
    }
}
```

### 待办事项API
- 端点：`/todo/list`
- 方法：POST
- 请求体：
```json
{
    "status": [0],
    "page": {
        "num": 1,
        "size": 5
    }
}
```

## 配置

编辑 `termclock.yml` 文件：

```yaml
# API配置
api_base_url: "http://10.20.0.26:8080"

# 温度传感器设备编号
device_code: "SENS-FARM01"

# 温度刷新频率（秒）
temp_refresh_interval: 5

# UI配置
# 时间字体缩放
time_scale_x: 2
time_scale_y: 2
date_scale_x: 1

# 颜色配置
time_color: "white"
date_color: "yellow"
todos_color: "white"

# 整点报时
chime_enabled: true

# 待办事项配置
todo_limit: 5
todo_task_max_chars: 68

# 界面布局
main_window_percent: 65
```

## 配置说明

### UI配置
- `time_scale_x`, `time_scale_y`: 时间字体缩放（X和Y方向）
- `date_scale_x`: 日期字体缩放
- `time_color`: 时间颜色（支持：white, red, green, yellow, blue, magenta, cyan, gray等）
- `date_color`: 日期颜色
- `todos_color`: 待办事项颜色
- `chime_enabled`: 是否启用整点报时

### API配置
- `api_base_url`: API服务器地址
- `device_code`: 温度传感器设备编号
- `temp_refresh_interval`: 温度刷新间隔（秒）

### 其他配置
- `main_window_percent`: 主窗口占屏幕百分比
- `todo_limit`: 待办事项显示数量限制
- `todo_task_max_chars`: 待办事项任务最大字符数

## 构建和运行

```bash
cargo build --release
./target/release/termclock
```

## 键盘快捷键

- `q` 或 `Esc` 或 `Ctrl+C`：退出程序
- `r`：刷新数据