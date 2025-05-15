# Jars - Java版本管理工具

一个简单的Java版本管理工具，支持安装、切换和列出Java版本。

## 安装

1. 确保已安装Rust工具链
2. 克隆本项目
3. 构建发布版本：
```bash
cargo build --release
```
4. 将`target/release/jars.exe`添加到系统PATH

## 使用

### 基本命令

```bash
jars [COMMAND]
```

### 可用命令

- `use <version>`: 设置当前使用的Java版本
- `list`: 列出所有已安装的Java版本
- `current`: 显示当前使用的Java版本
- `install <version>`: 安装指定版本的Java
- `help`: 显示帮助信息

### 示例

1. 安装Java 17:
```bash
jars install 17
```

2. 切换到Java 17:
```bash
jars use 17
```

3. 列出所有Java版本:
```bash
jars list
```

4. 显示当前Java版本:
```bash
jars current
```

## 特性

- 已安装的Java版本存放在: `~/.jars/versions/` (Windows: `%USERPROFILE%\.jars\versions\`)
- 自动从Adoptium下载Java
- 支持Windows、macOS和Linux
- 进度条显示下载进度
- 自动设置JAVA_HOME和PATH环境变量

## 依赖

- Rust 1.70+
- Windows: 需要访问注册表查询已安装的Java
- 其他平台: 检查标准Java安装路径

## 构建

```bash
cargo build --release
```

构建结果位于`target/release/jars`(或Windows上的`jars.exe`)

## TODO

- [x] Temurin JDK 的安装
- [ ] 其他 JDK 的支持（例如 Azul 等，仅需在 jar install **version** 添加一个 --provider）
- [ ] 一键切换 JDK 版本

## 鸣谢
[DeepSeek](https://deepseek.com)
