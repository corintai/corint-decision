# PostgreSQL 示例数据分析报告

## Feature 计算类型覆盖分析

### 支持的 Feature Operators

1. **Count** - 事件计数
2. **Sum** - 数值求和
3. **Avg** - 平均值
4. **Max/Min** - 最大/最小值
5. **CountDistinct** - 去重计数
6. **CrossDimensionCount** - 跨维度计数
7. **FirstSeen/LastSeen** - 首次/最后出现时间
8. **TimeSince** - 时间间隔
9. **Velocity** - 速度检查
10. **ProfileLookup** - 配置文件查找

### 当前数据问题

1. **数据量不足**：只有 3 个事件，无法测试各种时间窗口
2. **时间跨度不够**：所有事件都在同一时间段（2024-12-05 10:00-11:00）
3. **缺少关键事件类型**：
   - 缺少 `register` 事件（无法测试 account_age_days）
   - 缺少 `failed` login 事件（无法测试 failed_login_count_1h）
4. **缺少多样性数据**：
   - 缺少不同城市的测试数据
   - 缺少多个设备/IP 的关联数据
   - 缺少不同金额的交易数据
5. **无法测试时间窗口**：
   - 1h, 24h, 7d, 30d 窗口都需要不同时间点的数据

