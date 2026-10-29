<div align="center">

<img src="brand/logo.svg" width="96" height="96" alt="EasyTierUI">

# EasyTierUI

[EasyTier](https://github.com/EasyTier/EasyTier) 去中心化组网 VPN 的原生跨平台客户端。

macOS · Windows · Linux · iOS · Android · Web

[English](README.md) · **简体中文**

</div>

## 简介

EasyTierUI 让 EasyTier 网络的部署、配置与运行快速、正确且安全。它是基于 [Native Shell](https://nsh.dev)
的单一 Rust 应用,在各平台使用原生 UI 渲染——所有平台页面与功能一致,按可切换的角色组织:

|角色|功能|
|---|---|
|**节点**|在本机加入、创建、导入(TOML / 分享链接 / 二维码)并运行网络;查看对端、流量与脱敏日志|
|**控制台**|跨一台或多台 easytier-web 服务器管理设备、网络与凭据|
|**控制面托管**|在本地运行加固后的 easytier-web,或生成 Docker Compose / systemd 部署文件|

桌面端以独立进程运行官方 `easytier-core` / `easytier-web`,从 GitHub Releases 下载并校验 sha256,
支持多版本切换与回滚;移动端内嵌 EasyTier 库。

## 默认安全

- 默认开启加密,网络密钥由 CSPRNG 生成,机密存于系统钥匙串,绝不进入日志或命令行参数
- RPC 端口仅绑定回环地址;出口节点、子网代理、SOCKS5 等高风险功能默认关闭
- 托管的 easytier-web 在对外开放前替换默认密码并关闭注册
- 导入内容先预览、绝不自动启动;界面本身从不以提权方式运行

## 构建

需要 Rust(stable)与 `nsh` CLI;iOS / Android 还需 `protoc`。

```sh
nsh run                       # 在本机运行(--target ios|android|web|macos|linux|windows)
nsh test                      # cargo test + 无头 UI 测试
nsh package --target macos    # 打包(web、android、linux、windows、ios)
```

## 参与贡献

请先阅读 [AGENTS.md](AGENTS.md)(架构与规则)、[DESIGN.md](DESIGN.md) 与 [PRODUCT.md](PRODUCT.md)。
代码禁止 panic 与 `unsafe`;`cargo clippy --all-targets -- -D warnings` 必须无警告。
