# 古代浑仪机械传动误差仿真与天体指向精度分析系统

**面向天文史研究的宋代浑仪复原数字孪生平台**

---

## 目录

1. [系统架构](#系统架构)
2. [目录结构](#目录结构)
3. [快速部署](#快速部署)
4. [传感器模拟器](#传感器模拟器)
5. [API 接口](#api-接口)
6. [监控与告警](#监控与告警)
7. [数据存储策略](#数据存储策略)
8. [技术栈](#技术栈)

---

## 系统架构

```
                        ┌─────────────────────────────────────────────────────────────┐
                        │                       浏览器 / 监控面板                          │
                        │  (Three.js 3D 可视化 / Grafana / Prometheus UI)              │
                        └──────────────┬──────────────────────────┬──────────────────────┘
                                       │                          │
                                       │ 80                       │ 3000/9090
                                       ▼                          ▼
                        ┌─────────────────────────┐      ┌─────────────────────────┐
                        │   Nginx (前端静态资源)  │      │   Prometheus / Grafana  │
                        │   - Gzip 压缩          │      │   - 指标采集 / 可视化   │
                        │   - API 反向代理       │      └────────────┬────────────┘
                        └──────────┬──────────────┘                   │ :8081/metrics
                                   │ :8080                              │
                                   ▼                                    │
                        ┌─────────────────────────┐                     │
                        │   Rust Backend (Actix)  │◄────────────────────┘
                        │   - 4 模块 + mpsc 管道   │
                        │   - Tracing 结构化日志   │
                        │   - Prometheus 指标     │
                        └───────┬──────────┬──────┘
                                │          │ :1883 MQTT
                                │          ▼
                                │   ┌───────────────┐
                                │   │   EMQX Broker │◄──────────────┐
                                │   └───────────────┘               │
                                │                                   │
                                ▼                                   │
                        ┌─────────────────────┐                     │
                        │   ClickHouse (OLAP)  │                     │
                        │   - TTL 生命周期     │                     │
                        │   - 多级降采样        │                     │
                        └─────────────────────┘                     │
                                                                    │
                                                      ┌─────────────────────────┐
                                                      │   传感器模拟器 (Python)  │
                                                      │   - 齿轮磨损/间隙可调   │
                                                      │   - 5 种预设场景        │
                                                      │   - HTTP/MQTT 双通道    │
                                                      └─────────────────────────┘
```

### Rust 后端模块架构

```
              HTTP / MQTT 接收
                     │
                     ▼
            ┌──────────────────┐
            │   dtu_receiver   │ 校验 + 入库 + 1:3 广播
            └────────┬─────────┘
                     │ mpsc::channel
           ┌─────────┼──────────┐
           ▼         ▼          ▼
  ┌────────────┐ ┌─────────┐ ┌──────────┐
  │ transmission │ │ pointing │ │ alarm_ws │
  │  齿轮动力学   │ │ 天区精度  │ │ 告警评估 │
  │  Hertz 碰撞  │ │  ETC+DMF │ │ + WS 推送 │
  └──────┬──────┘ └────┬────┘ └────┬─────┘
         └──────────────┼────────────┘
                        ▼
                   ClickHouse 持久化
```

---

## 目录结构

```
.
├── backend/                         # Rust 后端
│   ├── src/
│   │   ├── main.rs                  # 管道装配 + 服务启动
│   │   ├── models.rs                # 数据模型 + 配置 + 消息枚举
│   │   ├── metrics.rs               # Prometheus 指标定义
│   │   ├── mqtt_ingest.rs           # MQTT 订阅器
│   │   ├── dtu_receiver.rs          # 模块1: 采集+校验
│   │   ├── transmission_simulator.rs# 模块2: 齿轮动力学
│   │   ├── pointing_analyzer.rs     # 模块3: 指向精度
│   │   ├── alarm_ws.rs              # 模块4: 告警+WS
│   │   ├── handlers.rs              # HTTP 路由
│   │   └── clickhouse.rs            # CH 客户端
│   ├── config/
│   │   ├── gear_params.json         # 齿轮参数配置
│   │   └── alarm_thresholds.json    # 告警阈值配置
│   ├── Dockerfile                   # 多阶段构建
│   └── Cargo.toml
├── frontend/                        # 前端 (Three.js)
│   ├── js/
│   │   ├── armillary_3d.js          # 3D 渲染模块
│   │   └── transmission_panel.js    # 面板+WS模块
│   ├── config/
│   │   └── visualization.json       # 可视化配置
│   ├── nginx.conf                   # Gzip + 反向代理
│   ├── Dockerfile
│   └── index.html
├── simulator/                       # 传感器模拟器
│   ├── hunyi_simulator.py           # 模拟器主程序
│   ├── requirements.txt
│   └── Dockerfile
├── clickhouse/
│   ├── init.sql                     # 建表 + 降采样视图
│   └── config.d/
│       └── config.xml               # CH 配置
├── observability/                   # 监控配置
│   ├── prometheus.yml
│   └── grafana/
│       └── datasources/
│           ├── prometheus.yml
│           └── clickhouse.yml
├── docker-compose.yml               # 一键编排
└── README.md
```

---

## 快速部署

### 前置条件
- Docker 24.0+
- Docker Compose v2+
- 至少 4GB 内存

### 一键启动

```bash
# 启动核心服务（前端 + 后端 + ClickHouse + MQTT）
docker compose up -d

# 启动核心服务 + 监控（Prometheus + Grafana）
docker compose --profile monitoring up -d

# 启动核心服务 + 模拟器（自动上报）
docker compose --profile simulator up -d
```

### 验证部署

```bash
# 查看服务状态
docker compose ps

# 查看日志
docker compose logs -f backend
docker compose logs -f simulator

# 健康检查
curl http://localhost:8080/health
curl http://localhost:8081/metrics

# 前端访问
open http://localhost/
```

### 端口映射

| 服务 | 端口 | 说明 |
|------|------|------|
| 前端 | 80 | Web 界面 |
| 后端 API | 8080 | REST API |
| Metrics | 8081 | Prometheus 指标 |
| ClickHouse | 8123/9000 | HTTP/原生协议 |
| EMQX MQTT | 1883 | MQTT 协议 |
| EMQX Dashboard | 18083 | MQTT 管理面板 |
| Prometheus | 9090 | 指标 UI |
| Grafana | 3000 | 监控面板 (admin/admin123) |

### 停止与清理

```bash
# 停止服务
docker compose down

# 停止并清理数据卷
docker compose down -v

# 仅停止监控
docker compose --profile monitoring down
```

---

## 传感器模拟器

### 功能特性

- ✅ 5 种预设磨损场景
- ✅ 独立控制 3 组齿轮初始磨损
- ✅ 齿轮间隙/啮合误差倍率调节
- ✅ 噪声倍率调节
- ✅ HTTP / MQTT 双协议上报（MQTT 失败自动回退 HTTP）
- ✅ 实时打印累积误差和告警

### 命令行参数

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `--device` | HUNYI-001 | 设备 ID |
| `--api` | http://backend:8080 | 后端 API 地址 |
| `--mqtt` | None | MQTT Broker 地址 (host:port) |
| `--mqtt-topic` | hunyi/sensor | MQTT 主题 |
| `--interval` | 60 | 上报间隔（秒） |
| `--count` | -1 | 最大上报次数 (-1 无限) |
| `--fast` | - | 快速模式 (1秒间隔) |
| `--wear-1` | 0.10 | 齿轮组 1 初始磨损 (0-1) |
| `--wear-2` | 0.08 | 齿轮组 2 初始磨损 (0-1) |
| `--wear-3` | 0.09 | 齿轮组 3 初始磨损 (0-1) |
| `--wear-rate` | 1.0 | 磨损增长倍率 |
| `--backlash` | 1.0 | 齿轮间隙/啮合误差倍率 |
| `--noise` | 1.0 | 噪声倍率 |
| `--profile` | - | 预设场景 (见下表) |

### 预设场景

| 场景 | wear | wear_rate | backlash | noise | 说明 |
|------|------|-----------|----------|-------|------|
| `normal` | (0.10, 0.08, 0.09) | 1.0 | 1.0 | 1.0 | 正常运行 |
| `worn` | (0.70, 0.65, 0.68) | 2.0 | 2.5 | 2.0 | 严重磨损 |
| `broken` | (0.95, 0.90, 0.92) | 5.0 | 5.0 | 4.0 | 损坏临界 |
| `cold` | (0.10, 0.08, 0.09) | 0.5 | 1.5 | 1.5 | 低温工况 |
| `hot` | (0.15, 0.12, 0.14) | 2.0 | 2.0 | 2.0 | 高温工况 |

### 使用示例

```bash
# Docker 方式（推荐）
docker compose run simulator --profile broken --fast --count 100

# 直接运行
cd simulator
pip install -r requirements.txt

# 快速模式 + 高温场景 + MQTT 上报
python hunyi_simulator.py --fast --profile hot \
    --mqtt mqtt:1883 --mqtt-topic hunyi/sensor

# 自定义初始磨损 + 高间隙 + 高噪声
python hunyi_simulator.py --interval 2 \
    --wear-1 0.5 --wear-2 0.45 --wear-3 0.55 \
    --backlash 3.0 --noise 2.5

# 模拟损坏临界状态（会快速触发告警）
python hunyi_simulator.py --profile broken --fast
```

---

## API 接口

### 传感器数据上报
```
POST /api/v1/sensor/ingest
Content-Type: application/json

{
  "device_id": "HUNYI-001",
  "axis_azimuth_angle": 45.0,
  "axis_elevation_angle": 60.0,
  "axis_equatorial_angle": 30.0,
  "gear_meshing_error_1": 0.15,
  ...
}
```

### 查询接口

| 接口 | 方法 | 说明 |
|------|------|------|
| `/health` | GET | 健康检查 |
| `/metrics` | GET | Prometheus 指标 |
| `/api/v1/transmission/errors` | GET | 传动误差历史 |
| `/api/v1/pointing/accuracy` | GET | 指向精度历史 |
| `/api/v1/alarms` | GET | 告警事件 |
| `/api/v1/gear/status` | GET | 齿轮状态 |
| `/ws` | WebSocket | 实时数据推送 |

### WebSocket 消息格式

```json
{
  "message_type": "sensor_reading",
  "payload": { ... },
  "timestamp": "2024-01-01T12:00:00Z"
}
```

message_type 取值：
- `sensor_reading` - 传感器读数
- `transmission_error` - 传动误差结果
- `pointing_accuracy` - 指向精度结果
- `alarm` - 告警事件

---

## 监控与告警

### Prometheus 指标

| 指标名称 | 类型 | 标签 | 说明 |
|----------|------|------|------|
| `hunyi_http_requests_total` | Counter | method, path, status | HTTP 请求计数 |
| `hunyi_http_request_duration_seconds` | Histogram | method, path | HTTP 请求耗时 |
| `hunyi_sensor_readings_received_total` | Counter | transport | 接收读数计数 (http/mqtt) |
| `hunyi_sensor_readings_valid_total` | Counter | device_id | 有效读数计数 |
| `hunyi_sensor_readings_invalid_total` | Counter | device_id, reason | 无效读数计数 |
| `hunyi_transmission_jobs_total` | Counter | axis_id | 传动仿真计数 |
| `hunyi_pointing_jobs_total` | Counter | sky_zone | 指向分析计数 |
| `hunyi_alarms_triggered_total` | Counter | alarm_type, alarm_level | 告警触发计数 |
| `hunyi_current_cumulative_error_arcmin_milli` | Gauge | - | 当前累积误差 (×1000) |
| `hunyi_current_gear_wear_permille` | Gauge | - | 当前平均磨损 (×1000) |
| `hunyi_mqtt_messages_received_total` | Counter | topic, status | MQTT 消息计数 |
| `process_*` | - | - | 进程级指标 (CPU/内存/FD) |

### Grafana 面板

访问 http://localhost:3000 (admin/admin123)

已预配置数据源：
- Prometheus: http://prometheus:9090
- ClickHouse: http://clickhouse:8123 (db: hunyi_analysis)

---

## 数据存储策略

### TTL 生命周期管理

| 表 / 视图 | 保留时间 | 说明 |
|-----------|----------|------|
| `sensor_readings` | 7 天 | 原始高频数据 |
| `transmission_error_analysis` | 30 天 | 传动误差明细 |
| `pointing_accuracy_analysis` | 30 天 | 指向精度明细 |
| `sensor_readings_1min_mv` | 3 个月 | 1 分钟降采样 |
| `sensor_readings_15min_mv` | 6 个月 | 15 分钟降采样 |
| `sensor_readings_1h_mv` | 1 年 | 1 小时降采样 |
| `pointing_daily_by_zone_mv` | 5 年 | 按天区按天聚合 |
| `alarm_events` | 3 年 | 告警事件 |
| `gear_status` | 1 年 | 齿轮状态 |

### 多级降采样流水线

```
原始数据 (sensor_readings, 1min)
    │
    ├─► SummingMergeTree 1min MV (保留 3 个月)
    │       │
    │       └─► SummingMergeTree 15min MV (保留 6 个月)
    │               │
    │               └─► SummingMergeTree 1h MV (保留 1 年)
    │
    └─► 指向精度分析 (pointing_accuracy_analysis)
            │
            └─► SummingMergeTree daily by sky_zone (保留 5 年)
```

---

## 技术栈

| 层次 | 技术 | 版本 |
|------|------|------|
| 后端 | Rust + Actix-web | 1.75 / 4.4 |
| 异步通信 | tokio mpsc | 1.35 |
| 数据库 | ClickHouse | 24.3 |
| MQTT | EMQX + rumqttc | 5.7 / 0.24 |
| 日志 | tracing + tracing-subscriber | 0.1 / 0.3 |
| 监控 | Prometheus + Grafana | 2.52 / 10.4 |
| 前端 | Three.js + ES Modules | 0.160 |
| 模拟器 | Python + paho-mqtt | 3.11 / 1.6 |
| 编排 | Docker Compose | v2 |
| 反向代理 | Nginx | 1.25 |

---

## 相关文档

- [宋史·天文志](https://ctext.org/wiki.pl?if=gb&res=601046&remap=gb) - 宋代浑仪原始记载
- [齿轮系统动力学](https://link.springer.com/book/10.1007/978-981-13-8315-7) - Hertz 接触理论参考
- [ClickHouse TTL 文档](https://clickhouse.com/docs/en/engines/table-engines/mergetree-family/mergetree#table-engines-mergetree-ttl)

---

**License**: MIT | 天文史研究专用
