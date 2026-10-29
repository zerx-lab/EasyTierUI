//! The control-plane pages in the web build: Status and Hardening of the easytier-web that serves the page, as far
//! as its REST API can read or change it.
//!
//! One field per string; `EN` and `ZH_CN` must both set every field. Templates use `{name}` placeholders.

/// Strings of this area in one language.
#[derive(Debug)]
pub struct WebHostStrings {
    /// Status: the REST API cell.
    pub sum_api: &'static str,
    /// Status: the API answered.
    pub api_reachable: &'static str,
    /// Status: the API did not answer.
    pub api_unreachable: &'static str,
    /// Status: caption under the API cell.
    pub api_caption: &'static str,
    /// Status: the account's device count cell.
    pub sum_devices: &'static str,
    /// Status: caption under the device count.
    pub devices_caption: &'static str,
    /// Status: the session count cell.
    pub sum_sessions: &'static str,
    /// Status: caption under the session count.
    pub sessions_caption: &'static str,
    /// Status: the core versions cell.
    pub sum_versions: &'static str,
    /// Status: the sessions panel title.
    pub section_sessions: &'static str,
    /// Status: the machine id column.
    pub col_machine: &'static str,
    /// Status: the hostname column.
    pub col_host: &'static str,
    /// Status: the account column.
    pub col_account: &'static str,
    /// Status: the location column.
    pub col_location: &'static str,
    /// Status: no machine session.
    pub sessions_empty_title: &'static str,
    /// Status: no machine session, body.
    pub sessions_empty_body: &'static str,
    /// Status: a read is loading.
    pub loading: &'static str,
    /// Status: a refresh failed while older figures show.
    pub refresh_failed: &'static str,

    /// Hardening: the intro.
    pub harden_intro: &'static str,
    /// Hardening: the checklist's title.
    pub section_checks: &'static str,
    /// Hardening: the check column.
    pub col_check: &'static str,
    /// Hardening: the status column.
    pub col_status: &'static str,
    /// Hardening: the password check.
    pub check_password: &'static str,
    /// Hardening: the password check's state.
    pub state_password: &'static str,
    /// Hardening: the password check's explanation.
    pub detail_password: &'static str,
    /// Hardening: the password action.
    pub action_password: &'static str,
    /// Hardening: registration off.
    pub check_registration: &'static str,
    /// Hardening: registration's explanation.
    pub detail_registration: &'static str,
    /// Hardening: the `/api/` proxy switch.
    pub check_proxy: &'static str,
    /// Hardening: the proxy's explanation.
    pub detail_proxy: &'static str,
    /// Hardening: auto-created accounts off.
    pub check_auto_create: &'static str,
    /// Hardening: auto-create's explanation.
    pub detail_auto_create: &'static str,
    /// Hardening: the seeded `user` account's password.
    pub check_seeded: &'static str,
    /// Hardening: the seeded account's explanation (never probed by signing in).
    pub detail_seeded: &'static str,
    /// Hardening: a check only deployment controls.
    pub state_deployment: &'static str,
    /// Hardening: the button to the Deployment page.
    pub action_deployment: &'static str,
}

pub(super) static EN: WebHostStrings = WebHostStrings {
    sum_api: "REST API",
    api_reachable: "Reachable",
    api_unreachable: "Not reachable",
    api_caption: "this server's easytier-web",
    sum_devices: "Devices",
    devices_caption: "connected for your account",
    sum_sessions: "Sessions",
    sessions_caption: "machines connected, every account",
    sum_versions: "Core versions",
    section_sessions: "Machine sessions",
    col_machine: "Machine ID",
    col_host: "Hostname",
    col_account: "Account",
    col_location: "Location",
    sessions_empty_title: "No machine sessions",
    sessions_empty_body: "No machine is connected to this server right now.",
    loading: "Reading the server…",
    refresh_failed: "The server did not answer. These figures are from the last successful read.",

    harden_intro: "This page can change your account's password through the server's API. What the server was \
                   started with (registration, the /api/ proxy, auto-created accounts, the seeded passwords) \
                   can't be read through it, so those rows say so instead of claiming a state.",
    section_checks: "Hardening",
    col_check: "Check",
    col_status: "Status",
    check_password: "Your account's password",
    state_password: "Yours to change",
    detail_password: "Changing it ends this session; you sign in again with the new password.",
    action_password: "Change password",
    check_registration: "Registration is off",
    detail_registration: "Whether anyone can register is easytier-web's --disable-registration flag. The API does \
                          not report it, so it can't be verified here. The generated deployment files turn it off.",
    check_proxy: "The /api/ proxy hardening switch",
    detail_proxy: "Which API paths the reverse proxy exposes is set in the deployment's proxy file. It can't be \
                   read from the browser; review it in Deployment.",
    check_auto_create: "Auto-created accounts are off",
    detail_auto_create: "Whether a sign-in with an unknown name creates an account is a server flag. The API does \
                         not report it; the generated deployment files keep it off.",
    check_seeded: "The seeded user account's password",
    detail_seeded: "easytier-web seeds a user account whose password is its name. This page never tries it: \
                    signing in as that account would replace this browser's session. Replace the password where \
                    the server is deployed.",
    state_deployment: "Set by deployment",
    action_deployment: "Open deployment",
};

pub(super) static ZH_CN: WebHostStrings = WebHostStrings {
    sum_api: "REST API",
    api_reachable: "可访问",
    api_unreachable: "无法访问",
    api_caption: "本服务器的 easytier-web",
    sum_devices: "设备",
    devices_caption: "当前账号已连接",
    sum_sessions: "会话",
    sessions_caption: "已连接的设备，含所有账号",
    sum_versions: "内核版本",
    section_sessions: "设备会话",
    col_machine: "设备 ID",
    col_host: "主机名",
    col_account: "账号",
    col_location: "位置",
    sessions_empty_title: "没有设备会话",
    sessions_empty_body: "目前没有设备连接到本服务器。",
    loading: "正在读取服务器…",
    refresh_failed: "服务器没有响应。以下数据来自上一次成功的读取。",

    harden_intro: "本页可通过服务器 API 修改当前账号的密码。服务器启动时的设置（注册、/api/ 代理、自动创建账号、\
                   预置密码）无法通过 API 读取，所以这些行只给出说明，不声称其状态。",
    section_checks: "加固",
    col_check: "检查项",
    col_status: "状态",
    check_password: "当前账号的密码",
    state_password: "由你修改",
    detail_password: "修改后本次会话会结束，需要用新密码重新登录。",
    action_password: "修改密码",
    check_registration: "已关闭注册",
    detail_registration: "是否允许注册由 easytier-web 的 --disable-registration 参数决定。API 不会报告它，所以无法在此验证。\
                          生成的部署文件会将其关闭。",
    check_proxy: "/api/ 代理加固开关",
    detail_proxy: "反向代理开放哪些 API 路径由部署中的代理配置决定，浏览器无法读取。请在“部署”中查看。",
    check_auto_create: "已关闭自动创建账号",
    detail_auto_create: "用未知用户名登录时是否自动创建账号，由服务器参数决定。API 不会报告它；生成的部署文件会将其关闭。",
    check_seeded: "预置 user 账号的密码",
    detail_seeded: "easytier-web 会预置一个密码与用户名相同的 user 账号。本页绝不会尝试用它登录：\
                    以该账号登录会替换浏览器当前的会话。请在部署服务器的地方更换其密码。",
    state_deployment: "由部署决定",
    action_deployment: "打开部署",
};
