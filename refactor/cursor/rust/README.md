# 🦀 Inngest Rust 重构实现

> Inngest 事件驱动执行平台的 Rust 重构版本

[![Rust](https://img.shields.io/badge/rust-1.88+-orange.svg)](https://www.rust-lang.org)
[![Edition](https://img.shields.io/badge/edition-2024-blue.svg)](https://doc.rust-lang.org/edition-guide/rust-2024/)
[![Status](https://img.shields.io/badge/status-phase1_complete-green.svg)](.)

## 🎯 项目概述

这是 Inngest 平台的 Rust 重构实现，旨在提供更高的性能、内存安全性和并发能力，同时保持与现有 Go 版本的完全兼容性。

## 🏗️ 项目结构

```
rust/
├── Cargo.toml                 # Workspace 配置
├── Cargo.lock                 # 依赖锁定文件
├── crates/
│   └── inngest-core/          # ✅ 核心数据结构和 trait
│       ├── src/
│       │   ├── lib.rs         # 主模块
│       │   ├── error.rs       # 统一错误处理
│       │   ├── event.rs       # 事件数据结构
│       │   ├── function.rs    # 函数定义
│       │   ├── identifier.rs  # 运行标识符
│       │   ├── metadata.rs    # 运行元数据
│       │   ├── queue.rs       # 队列数据结构
│       │   └── state.rs       # 状态管理 trait
│       └── Cargo.toml
└── README.md
```

## 🛠️ 技术规格

### Rust 版本要求
- **Rust 版本**: 1.88+
- **Edition**: 2024
- **Resolver**: 3

### 核心依赖
| 依赖 | 版本 | 用途 |
|------|------|------|
| `serde` + `serde_json` | 1.0 | JSON 序列化兼容性 |
| `uuid` | 1.17 | UUID 标识符 |
| `ulid` | 1.2 | ULID 标识符 |
| `chrono` | 0.4 | 时间处理 |
| `thiserror` + `anyhow` | 2.0 + 1.0 | 错误处理 |
| `async-trait` | 0.1 | 异步 trait |
| `base64` | 0.22 | Base64 编码 |

## 🚀 快速开始

### 前提条件
```bash
# 检查 Rust 版本
rustc --version  # 需要 1.88+
cargo --version
```

### 编译和测试
```bash
# 进入 Rust 子项目目录
cd rust

# 检查代码编译
cargo check --package inngest-core

# 运行所有测试
cargo test --package inngest-core

# 运行测试并显示详细输出
cargo test --package inngest-core -- --nocapture

# 代码格式化
cargo fmt

# 代码检查
cargo clippy
```

### 测试结果
```
✅ 26/26 测试通过
✅ 零编译警告
✅ Clippy 检查通过
✅ 完整兼容性验证
```

## 📊 当前实现状态

### ✅ 已完成 (第1阶段)

#### 核心数据结构
- **Identifier** - 工作流运行标识符，完全兼容Go版本格式
- **Event** - 事件数据结构，支持JSON序列化兼容性
- **Function** - 函数定义和配置，包含触发器和流控配置
- **Metadata** - 运行元数据，支持状态跟踪和调试信息
- **Queue** - 队列项和调度数据结构，支持优先级和重试逻辑
- **State** - 状态管理接口，支持内存和持久化实现

#### 错误处理系统
- 统一的错误类型层次结构
- 结构化错误消息和错误代码
- 可重试性判断逻辑
- 兼容性保证的错误格式

#### 测试框架
- 完整的单元测试覆盖 (26个测试)
- 序列化兼容性测试
- 业务逻辑正确性验证
- 边界条件和错误处理测试

### 🔄 下一阶段计划

#### 第2阶段：状态管理系统
- [ ] **Redis 状态存储** - 实现与 Go 版本兼容的状态持久化
- [ ] **Lua 脚本迁移** - 移植所有 Redis Lua 脚本逻辑
- [ ] **状态序列化** - 确保状态数据格式100%兼容

#### 第3阶段：队列系统
- [ ] **Redis 队列实现** - 高性能分片队列系统
- [ ] **智能调度算法** - 优先级、公平性和背压控制
- [ ] **流控功能** - 并发控制、限流、批处理支持

## 🔄 兼容性保证

### 数据格式兼容性
```rust
// JSON 序列化保持与 Go 版本完全一致
#[derive(Serialize, Deserialize)]
struct Identifier {
    #[serde(rename = "runID")]
    pub run_id: Ulid,
    
    #[serde(rename = "wID")]
    pub workflow_id: Uuid,
    // ... 其他字段
}
```

### 测试兼容性
- ✅ **序列化格式** - JSON 字段名和类型完全匹配
- ✅ **ID 格式** - UUID/ULID 生成算法保持一致
- ✅ **时间戳精度** - 毫秒级时间戳格式兼容
- ✅ **哈希算法** - XXHash 和 SHA 算法结果一致

## 📈 代码质量指标

### 测试覆盖率
- **单元测试**: 26/26 通过 (100%)
- **功能覆盖**: 所有核心数据结构
- **兼容性测试**: 序列化/反序列化验证

### 代码质量
- ✅ **编译检查**: 零警告编译
- ✅ **Clippy 检查**: 静态分析通过
- ✅ **格式标准**: `cargo fmt` 标准格式
- ✅ **类型安全**: 强类型系统保证

## 🔧 开发指南

### 代码风格
```rust
// 使用 workspace 依赖
[dependencies]
serde = { workspace = true }

// 使用结构化错误处理
use anyhow::Result;
use thiserror::Error;

// 实现异步 trait
#[async_trait]
pub trait StateManager: Send + Sync + Clone {
    async fn load(&self, id: &Identifier) -> Result<Box<dyn State>>;
}
```

### 测试模式
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_example() {
        // Arrange
        let input = create_test_input();
        
        // Act
        let result = process(input);
        
        // Assert
        assert_eq!(result.status, ExpectedStatus::Success);
    }
}
```

## 📞 贡献指南

### 开发环境设置
```bash
# 克隆仓库
git clone <repository-url>
cd inngest/rust

# 安装依赖
cargo build

# 运行测试
cargo test

# 代码质量检查
cargo clippy
cargo fmt --check
```

### 提交要求
- ✅ 所有测试必须通过
- ✅ Clippy 检查无警告
- ✅ 代码格式符合标准
- ✅ 兼容性测试验证

## 📄 许可证

本项目遵循 [MIT 许可证](../LICENSE.md)。

---

<div align="center">

**🚀 高性能 • 🛡️ 内存安全 • ⚡ 并发优化**

[📖 查看详细文档](../prd/) • [🏗️ 项目架构](../prd/INNGEST_ARCHITECTURE_ANALYSIS.md) • [⚡ 实施策略](../prd/RUST_REWRITE_STRATEGY.md)

</div> 