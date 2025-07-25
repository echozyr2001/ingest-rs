# 🦀 Inngest Rust 重构项目

> 将 Inngest 事件驱动执行平台从 Go 重构为 Rust，提升性能、安全性和并发能力

[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![Status](https://img.shields.io/badge/status-planning-blue.svg)](https://github.com/inngest/inngest)
[![License](https://img.shields.io/badge/license-Apache%202.0-green.svg)](LICENSE)

## 🎯 项目目标

将 Inngest 的 Go 实现完全用 Rust 重写，实现：

- 🚀 **性能提升** - 更高的事件处理吞吐量
- 🛡️ **内存安全** - 零内存泄漏和数据竞争
- ⚡ **并发优化** - 更安全、更高效的并发模型  
- 🔧 **维护性增强** - 强类型系统减少运行时错误
- 🔄 **完全兼容** - 与现有系统 100% 兼容

## 📋 快速导航

| 🎯 想要... | 📖 阅读文档 |
|---------|----------|
| **了解项目全貌** | → [项目主文档](./INNGEST_RUST_REWRITE_MASTER.md) |
| **理解系统架构** | → [架构分析](./INNGEST_ARCHITECTURE_ANALYSIS.md) |
| **深入组件实现** | → [组件详解](./INNGEST_COMPONENT_DETAILS.md) |
| **查看实施计划** | → [重构策略](./RUST_REWRITE_STRATEGY.md) |

## 🏗️ 系统架构

```
┌─────────────────────────────────────────────────────────────┐
│                     Inngest 系统架构                          │
├─────────────────────────────────────────────────────────────┤
│  入口层:  Event API  │  Core API  │  Connect Gateway        │
├─────────────────────────────────────────────────────────────┤  
│  服务层:  Runner Service  │  Dev Server  │  CQRS Service   │
├─────────────────────────────────────────────────────────────┤
│  执行层:  Executor  │  Queue System  │  State Manager      │
├─────────────────────────────────────────────────────────────┤
│  存储层:  Redis (队列/状态)  │  PostgreSQL (元数据)          │
└─────────────────────────────────────────────────────────────┘
```

## 🚀 实施路线图

### 📅 8 个开发阶段

| 阶段 | 核心内容 | 状态 |
|------|----------|------|
| **Phase 1** | 基础设施 + 核心数据结构 | 📋 计划中 |
| **Phase 2** | 状态管理系统 | ⏳ 待开始 |
| **Phase 3** | 队列系统 | ⏳ 待开始 |
| **Phase 4** | 执行引擎 | ⏳ 待开始 |
| **Phase 5** | API 层 + Runner | ⏳ 待开始 |
| **Phase 6** | Connect 系统 | ⏳ 待开始 |
| **Phase 7** | CQRS + DevServer | ⏳ 待开始 |
| **Phase 8** | 测试 + 优化 | ⏳ 待开始 |

### 🎯 关键里程碑

- ✅ **M0**: 项目规划和文档完成
- 🎯 **M1**: 状态管理系统兼容 (第2阶段)
- 🎯 **M2**: 核心执行引擎可用 (第4阶段)  
- 🎯 **M3**: 功能完整验证 (第6阶段)
- 🎯 **M4**: 生产环境就绪 (第8阶段)

## 🛠️ 技术栈

### Rust 生态选择

```toml
[dependencies]
# 异步运行时
tokio = { version = "1.0", features = ["full"] }

# HTTP 服务
axum = "0.8"

# 数据库
sqlx = { version = "0.8", features = ["postgres", "sqlite", "runtime-tokio"] }

# Redis
fred = "10.0"

# WebSocket  
tokio-tungstenite = "0.27"

# 序列化
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# 错误处理
anyhow = "1.0"
thiserror = "2.0"
```

### 项目结构

```
inngest-rust/
├── 📦 crates/
│   ├── inngest-core/          # 核心抽象和数据结构
│   ├── inngest-state/         # Redis 状态管理
│   ├── inngest-queue/         # 高性能队列系统
│   ├── inngest-executor/      # 函数执行引擎
│   ├── inngest-connect/       # WebSocket 网关
│   └── inngest-cli/           # 命令行工具
├── 🧪 tests/                  # 集成测试
├── 📋 proto/                  # Protobuf 定义
└── 📚 docs/                   # 项目文档
```

## ⚡ 性能优化重点

- **事件吞吐量**: 利用 Rust 异步特性和零成本抽象优化事件处理
- **响应延迟**: 通过编译时优化和内存管理改善响应速度
- **内存使用**: 通过 ownership 系统实现更精确的内存控制
- **CPU 使用率**: 减少运行时开销，提高计算效率

## 🔒 兼容性承诺

### 完全向后兼容

- ✅ **数据格式**: Redis 和数据库数据格式完全兼容
- ✅ **API 接口**: HTTP 和 WebSocket API 保持一致
- ✅ **SDK 支持**: 现有 TypeScript/Python/Go SDK 无需修改
- ✅ **配置文件**: 支持现有配置格式
- ✅ **部署脚本**: 兼容现有部署流程

## 🧪 质量保证

### 测试策略
- **单元测试**: 覆盖率 > 85%
- **集成测试**: 覆盖率 > 90%  
- **兼容性测试**: 与 Go 版本功能对比
- **性能测试**: 持续基准测试
- **负载测试**: 高并发场景验证

### 开发流程
- **代码审查**: 强制 PR 审查
- **CI/CD**: 自动化测试和部署
- **文档同步**: 代码和文档同步更新
- **性能监控**: 每个版本性能基准

## 🚦 项目状态

### 当前进展

- ✅ **系统架构分析** - 完成对现有 Go 系统的深度分析
- ✅ **技术选型** - 确定 Rust 技术栈和工具链
- ✅ **实施计划** - 详细的 8 阶段开发计划
- ✅ **兼容性设计** - 保证与现有系统兼容的设计方案
- 🔄 **开发环境准备** - 正在设置开发和测试环境

### 近期计划

- 🎯 **第1阶段启动**
  - 建立 Rust 项目结构
  - 实现核心数据结构
  - 设置 CI/CD 流水线
  - 建立测试框架

## 📞 联系方式

- **项目讨论**: [GitHub Discussions](https://github.com/inngest/inngest/discussions)
- **问题报告**: [GitHub Issues](https://github.com/inngest/inngest/issues)
- **开发进展**: [项目 Wiki](https://github.com/inngest/inngest/wiki)

## 📄 许可证

本项目遵循 [Apache 2.0 许可证](LICENSE)。

---

<div align="center">

**🚀 Ready to build the future of event-driven infrastructure with Rust? Let's go!**

[📖 阅读完整文档](./INNGEST_RUST_REWRITE_MASTER.md) • [🏗️ 查看架构](./INNGEST_ARCHITECTURE_ANALYSIS.md) • [⚡ 实施策略](./RUST_REWRITE_STRATEGY.md)

</div> 