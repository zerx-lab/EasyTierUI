//! Words every area shares: actions, states and the age of what the app shows.
//!
//! One field per string; `EN` and `ZH_CN` must both set every field. Templates use `{name}` placeholders filled by
//! [`super::fill`].

/// Shared strings in one language.
#[derive(Debug)]
pub struct CommonStrings {
    /// Confirms a dialog.
    pub ok: &'static str,
    /// Dismisses a dialog or form without changes.
    pub cancel: &'static str,
    /// Saves a form.
    pub save: &'static str,
    /// Deletes something (in a confirmation that names it).
    pub delete: &'static str,
    /// Edits something.
    pub edit: &'static str,
    /// Copies a value to the clipboard.
    pub copy: &'static str,
    /// After copying.
    pub copied: &'static str,
    /// Tries a failed action again.
    pub retry: &'static str,
    /// Closes a panel or dialog.
    pub close: &'static str,
    /// Reveals a masked secret.
    pub show: &'static str,
    /// Masks a revealed secret.
    pub hide: &'static str,
    /// Generates a strong random secret.
    pub generate: &'static str,
    /// Something is loading.
    pub loading: &'static str,
    /// Re-reads something now.
    pub refresh: &'static str,
    /// Moves on in a multi-step flow.
    pub next: &'static str,
    /// Lamp word: running.
    pub state_running: &'static str,
    /// Lamp word: starting.
    pub state_starting: &'static str,
    /// Lamp word: stopping.
    pub state_stopping: &'static str,
    /// Lamp word: stopped.
    pub state_stopped: &'static str,
    /// Lamp word: failed.
    pub state_failed: &'static str,
    /// Lamp word: signed in to a server.
    pub state_online: &'static str,
    /// Lamp word: connecting or signing in.
    pub state_connecting: &'static str,
    /// The age of a reading: `{age}` is whole seconds (`age_text` fills it).
    pub updated_ago: &'static str,
    /// The age of a reading taken just now.
    pub updated_now: &'static str,
    /// A secret field's strength hint: weak.
    pub strength_weak: &'static str,
    /// A secret field's strength hint: fair.
    pub strength_fair: &'static str,
    /// A secret field's strength hint: strong.
    pub strength_strong: &'static str,
    /// Secrets are kept only until the app quits (no keychain on this platform).
    pub secrets_not_kept: &'static str,
    /// A required field is empty: `{field}` names it.
    pub field_required: &'static str,
}

pub(super) static EN: CommonStrings = CommonStrings {
    ok: "OK",
    cancel: "Cancel",
    save: "Save",
    delete: "Delete",
    edit: "Edit",
    copy: "Copy",
    copied: "Copied",
    retry: "Try again",
    close: "Close",
    show: "Show",
    hide: "Hide",
    generate: "Generate",
    loading: "Loading…",
    refresh: "Refresh",
    next: "Next",
    state_running: "Running",
    state_starting: "Starting",
    state_stopping: "Stopping",
    state_stopped: "Stopped",
    state_failed: "Failed",
    state_online: "Signed in",
    state_connecting: "Connecting",
    updated_ago: "Updated {age} s ago",
    updated_now: "Updated just now",
    strength_weak: "Weak: easy to guess. Use Generate or at least 20 mixed characters.",
    strength_fair: "Fair. A generated secret is stronger.",
    strength_strong: "Strong",
    secrets_not_kept: "This device has no keychain the app can use: secrets are kept only until the app quits.",
    field_required: "{field} is required.",
};

pub(super) static ZH_CN: CommonStrings = CommonStrings {
    ok: "好",
    cancel: "取消",
    save: "保存",
    delete: "删除",
    edit: "编辑",
    copy: "复制",
    copied: "已复制",
    retry: "重试",
    close: "关闭",
    show: "显示",
    hide: "隐藏",
    generate: "生成",
    loading: "正在加载…",
    refresh: "刷新",
    next: "下一步",
    state_running: "运行中",
    state_starting: "正在启动",
    state_stopping: "正在停止",
    state_stopped: "未运行",
    state_failed: "失败",
    state_online: "已登录",
    state_connecting: "正在连接",
    updated_ago: "{age} 秒前更新",
    updated_now: "刚刚更新",
    strength_weak: "弱：容易被猜到。请使用“生成”，或至少 20 个混合字符。",
    strength_fair: "一般。生成的密钥更安全。",
    strength_strong: "强",
    secrets_not_kept: "此设备没有可用的系统钥匙串：密钥只保留到应用退出。",
    field_required: "请填写{field}。",
};
