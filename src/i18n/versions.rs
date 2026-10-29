//! EasyTier programs on this desktop: detected versions, releases, download, verify, switch, roll back.
//!
//! One field per string; `EN` and `ZH_CN` must both set every field (the struct makes a missing one a compile
//! error). Templates use `{name}` placeholders filled by [`super::fill`].

/// Strings of this area in one language.
#[derive(Debug)]
pub struct VersionStrings {
    /// Settings section: the EasyTier programs.
    pub section: &'static str,
    /// Row label: the release the next start runs.
    pub active_label: &'static str,
    /// Page header action: look again for releases, here and on GitHub.
    pub check_updates: &'static str,
    /// Summary label: the number of releases on this device.
    pub installed_summary: &'static str,
    /// Summary unit after the count of installed releases.
    pub unit_releases: &'static str,
    /// Summary label: the newest release GitHub lists.
    pub latest_label: &'static str,
    /// Summary caption: a newer release than the active one can be installed.
    pub update_available: &'static str,
    /// Summary caption: the active release is the newest.
    pub up_to_date: &'static str,
    /// Table header: the state of an installed release.
    pub col_status: &'static str,
    /// Table header: the date a release was published.
    pub col_published: &'static str,
    /// Table header: the size of a release's download.
    pub col_size: &'static str,
    /// Table header: how a download is checked.
    pub col_verification: &'static str,
    /// Empty installed table: title.
    pub installed_empty_title: &'static str,
    /// Lamp word: the active release works.
    pub status_ready: &'static str,
    /// Lamp word: the active release is older than the app supports.
    pub status_unsupported: &'static str,
    /// Lamp word: no release is installed.
    pub status_missing: &'static str,
    /// Lamp word: the active release could not be read.
    pub status_unreadable: &'static str,
    /// Nothing is installed; what to do.
    pub missing_body: &'static str,
    /// The active release is too old; what to do.
    pub unsupported_body: &'static str,
    /// Row label before the oldest supported release.
    pub oldest_label: &'static str,
    /// Section label: releases on this device.
    pub installed_label: &'static str,
    /// No release is installed.
    pub installed_empty: &'static str,
    /// Chip: the active release.
    pub badge_active: &'static str,
    /// Chip: the release a roll back returns to.
    pub badge_previous: &'static str,
    /// Chip: a directory the user chose.
    pub badge_custom: &'static str,
    /// Lamp word: an installed release that cannot run.
    pub badge_damaged: &'static str,
    /// Makes an installed release the active one.
    pub activate: &'static str,
    /// Accessible name of an Activate button: `{version}`.
    pub activate_named: &'static str,
    /// Deletes an installed release.
    pub remove: &'static str,
    /// Accessible name of a Remove button: `{version}`.
    pub remove_named: &'static str,
    /// Goes back to the previous release.
    pub rollback: &'static str,
    /// Accessible name of the Roll back button: `{version}`.
    pub rollback_to: &'static str,
    /// What switching does to running networks and the hosted control plane.
    pub switch_note: &'static str,
    /// While a switch is in progress.
    pub switching: &'static str,
    /// A directory's name before the path.
    pub directory_label: &'static str,

    /// Section label: releases on GitHub.
    pub releases_label: &'static str,
    /// Where releases come from and how they are checked.
    pub releases_intro: &'static str,
    /// No release listed.
    pub releases_empty: &'static str,
    /// Chip: a pre-release.
    pub prerelease: &'static str,
    /// Chip: already on this device.
    pub installed_badge: &'static str,
    /// Meta: the download has a sha256 digest to check.
    pub verified: &'static str,
    /// Downloads and installs a release (does not activate it).
    pub install: &'static str,
    /// Accessible name of an Install button: `{version}`.
    pub install_named: &'static str,
    /// Why a release cannot be installed: older than supported.
    pub blocker_too_old: &'static str,
    /// Why: no zip for this device.
    pub blocker_no_download: &'static str,
    /// Why: no sha256 digest published.
    pub blocker_no_digest: &'static str,
    /// Progress word: preparing.
    pub phase_starting: &'static str,
    /// Progress word: downloading.
    pub phase_downloading: &'static str,
    /// Progress word: checking the sha256.
    pub phase_verifying: &'static str,
    /// Progress word: unpacking.
    pub phase_unpacking: &'static str,
    /// After a version number: the release was installed.
    pub installed_ready: &'static str,

    /// Confirmation title before installing: `{version}`.
    pub confirm_install_title: &'static str,
    /// Confirmation text before installing: `{size}`.
    pub confirm_install_body: &'static str,
    /// Confirmation title before removing: `{version}`.
    pub confirm_remove_title: &'static str,
    /// Confirmation text before removing.
    pub confirm_remove_body: &'static str,
    /// Confirmation title before using a release found on disk: `{version}`.
    pub confirm_adopt_title: &'static str,
    /// Confirmation text before using a release found on disk: `{path}`.
    pub confirm_adopt_body: &'static str,
    /// Confirms using a release found on disk.
    pub confirm_adopt_action: &'static str,

    /// Row label: a release found on the PATH.
    pub path_label: &'static str,
    /// While looking.
    pub path_searching: &'static str,
    /// Nothing found.
    pub path_none: &'static str,
    /// Starts the confirmation to use a release found on disk.
    pub adopt: &'static str,
    /// Accessible name of the adopt button: `{version}`.
    pub adopt_named: &'static str,
    /// Chip: a release too old to use.
    pub adopt_too_old: &'static str,
    /// Section label: choose a directory.
    pub custom_label: &'static str,
    /// What a custom directory must hold.
    pub custom_help: &'static str,
    /// Field label: the directory path.
    pub custom_field: &'static str,
    /// Checks the directory.
    pub custom_check: &'static str,
    /// While checking.
    pub custom_checking: &'static str,

    /// Mobile: label of the library's version.
    pub embedded_label: &'static str,
    /// Mobile: how EasyTier is delivered.
    pub embedded_note: &'static str,
    /// Web: how the server's programs are updated.
    pub web_note: &'static str,
    /// Web: opens the deployment files.
    pub web_action: &'static str,

    /// Problem: GitHub unreachable.
    pub problem_offline: &'static str,
    /// Problem: GitHub's rate limit.
    pub problem_rate_limited: &'static str,
    /// Problem: unexpected HTTP status (the status follows in monospace).
    pub problem_http: &'static str,
    /// Problem: unreadable answer.
    pub problem_bad_response: &'static str,
    /// Problem: no download for this device.
    pub problem_no_download: &'static str,
    /// Problem: no digest.
    pub problem_missing_digest: &'static str,
    /// Problem: too large.
    pub problem_too_large: &'static str,
    /// Problem: size differs from the listing.
    pub problem_size_mismatch: &'static str,
    /// Problem: sha256 differs.
    pub problem_digest_mismatch: &'static str,
    /// Problem: too old (the version follows in monospace).
    pub problem_unsupported: &'static str,
    /// Problem: damaged archive.
    pub problem_bad_archive: &'static str,
    /// Problem: archive with a forbidden entry.
    pub problem_unsafe_archive: &'static str,
    /// Problem: a program is missing (its name follows in monospace).
    pub problem_missing_program: &'static str,
    /// Problem: a program reported no version (its name follows in monospace).
    pub problem_version_unreadable: &'static str,
    /// Problem: the download is another release than requested.
    pub problem_version_mismatch: &'static str,
    /// Problem: already installed (the version follows in monospace).
    pub problem_already_installed: &'static str,
    /// Problem: not installed (the version follows in monospace).
    pub problem_not_installed: &'static str,
    /// Problem: the active release cannot be removed.
    pub problem_active_in_use: &'static str,
    /// Problem: nothing to go back to.
    pub problem_no_previous: &'static str,
    /// Problem: no core on the PATH.
    pub problem_not_on_path: &'static str,
    /// Problem: the path is not absolute.
    pub problem_not_absolute: &'static str,
    /// Problem: a file operation failed (the OS message follows in monospace).
    pub problem_disk: &'static str,
    /// Problem: the data folder is unusable (the OS message follows in monospace).
    pub problem_data_dir: &'static str,
    /// Problem: the background task ended without a result.
    pub problem_interrupted: &'static str,
    /// Problem: not available on this platform.
    pub problem_unavailable: &'static str,
}

pub(super) static EN: VersionStrings = VersionStrings {
    section: "EasyTier",
    active_label: "Active release",
    check_updates: "Check for updates",
    installed_summary: "Installed",
    unit_releases: "releases",
    latest_label: "Latest release",
    update_available: "Update available",
    up_to_date: "Up to date",
    col_status: "Status",
    col_published: "Published",
    col_size: "Size",
    col_verification: "Verification",
    installed_empty_title: "Nothing installed yet",
    status_ready: "Ready",
    status_unsupported: "Not supported",
    status_missing: "Not installed",
    status_unreadable: "Can't be read",
    missing_body: "No EasyTier release is installed on this device. Install one below to join networks or host a \
                   control plane.",
    unsupported_body: "The active release is older than this app supports. Install a newer one below.",
    oldest_label: "Oldest supported release",
    installed_label: "On this device",
    installed_empty: "Releases you install are kept side by side, so you can go back to an earlier one.",
    badge_active: "Active",
    badge_previous: "Previous",
    badge_custom: "Your own directory",
    badge_damaged: "Can't run",
    activate: "Activate",
    activate_named: "Activate {version}",
    remove: "Remove",
    remove_named: "Remove {version}",
    rollback: "Roll back",
    rollback_to: "Roll back to {version}",
    switch_note: "Switching stops running networks and the hosted control plane, then starts them again with the \
                  new release.",
    switching: "Switching…",
    directory_label: "Directory",

    releases_label: "Available releases",
    releases_intro: "Releases come from EasyTier's GitHub page over HTTPS. Each download is checked against the \
                     release's sha256 digest before anything is unpacked, and nothing is installed without your \
                     say-so.",
    releases_empty: "GitHub lists no releases.",
    prerelease: "Pre-release",
    installed_badge: "Installed",
    verified: "Verified by sha256 digest",
    install: "Install",
    install_named: "Install {version}",
    blocker_too_old: "Older than this app supports",
    blocker_no_download: "No download for this device",
    blocker_no_digest: "No sha256 digest published, so it can't be verified",
    phase_starting: "Preparing…",
    phase_downloading: "Downloading…",
    phase_verifying: "Verifying the sha256 digest…",
    phase_unpacking: "Unpacking…",
    installed_ready: "is installed and ready to activate.",

    confirm_install_title: "Install EasyTier {version}?",
    confirm_install_body: "Downloads about {size} from GitHub and checks its sha256 digest before unpacking. It is \
                           installed next to your other releases and not activated until you choose to.",
    confirm_remove_title: "Remove EasyTier {version}?",
    confirm_remove_body: "Its files are deleted from this device. Your other releases are not affected.",
    confirm_adopt_title: "Use EasyTier {version}?",
    confirm_adopt_body: "From {path}. Running networks restart with it. The files stay where they are, and the app \
                         does not update or remove them.",
    confirm_adopt_action: "Use this release",

    path_label: "Found on your PATH",
    path_searching: "Looking for easytier-core on your PATH…",
    path_none: "No easytier-core was found on your PATH.",
    adopt: "Use this release…",
    adopt_named: "Use release {version}",
    adopt_too_old: "Older than this app supports",
    custom_label: "A release you installed yourself",
    custom_help: "Enter the absolute path of a directory that holds easytier-core, easytier-cli and easytier-web.",
    custom_field: "Directory",
    custom_check: "Check directory",
    custom_checking: "Checking…",

    embedded_label: "Library version",
    embedded_note: "EasyTier is built into this app and updates with it.",
    web_note: "The EasyTier programs on this server are updated through its deployment files.",
    web_action: "Open deployment",

    problem_offline: "Couldn't reach GitHub. Check the connection and try again.",
    problem_rate_limited: "GitHub's rate limit was reached. Try again in a few minutes.",
    problem_http: "GitHub answered with an unexpected status.",
    problem_bad_response: "GitHub's answer couldn't be read.",
    problem_no_download: "This release has no download for this device.",
    problem_missing_digest: "This release publishes no sha256 digest, so it can't be verified and is not installed.",
    problem_too_large: "The download is larger than the app accepts.",
    problem_size_mismatch: "The download's size isn't the one GitHub lists. Nothing was installed.",
    problem_digest_mismatch: "The download doesn't match the release's sha256 digest. Nothing was installed.",
    problem_unsupported: "That release is older than this app supports.",
    problem_bad_archive: "The download is damaged. Nothing was installed.",
    problem_unsafe_archive: "The download holds a file that isn't allowed. Nothing was installed.",
    problem_missing_program: "A program of that release is missing.",
    problem_version_unreadable: "A program didn't report its version.",
    problem_version_mismatch: "The downloaded program is not the release that was requested. Nothing was installed.",
    problem_already_installed: "That release is already installed.",
    problem_not_installed: "That release is not installed.",
    problem_active_in_use: "The active release can't be removed. Activate another one first.",
    problem_no_previous: "There is no previous release to go back to.",
    problem_not_on_path: "No easytier-core was found on your PATH.",
    problem_not_absolute: "Enter an absolute directory path.",
    problem_disk: "A file on this device couldn't be read or written.",
    problem_data_dir: "The app's data folder can't be used.",
    problem_interrupted: "The task ended without a result. Try again.",
    problem_unavailable: "This isn't available on this platform.",
};

pub(super) static ZH_CN: VersionStrings = VersionStrings {
    section: "EasyTier",
    active_label: "当前使用的版本",
    check_updates: "检查更新",
    installed_summary: "已安装",
    unit_releases: "个版本",
    latest_label: "最新版本",
    update_available: "有可用更新",
    up_to_date: "已是最新",
    col_status: "状态",
    col_published: "发布日期",
    col_size: "大小",
    col_verification: "校验",
    installed_empty_title: "尚未安装任何版本",
    status_ready: "可用",
    status_unsupported: "不受支持",
    status_missing: "未安装",
    status_unreadable: "无法读取",
    missing_body: "此设备尚未安装 EasyTier。请在下方安装一个版本，才能加入网络或托管控制平面。",
    unsupported_body: "当前版本低于本应用支持的最低版本，请在下方安装更新的版本。",
    oldest_label: "最低支持版本",
    installed_label: "已安装在此设备",
    installed_empty: "安装的版本会并存保留，随时可以切换回之前的版本。",
    badge_active: "使用中",
    badge_previous: "上一个",
    badge_custom: "自选目录",
    badge_damaged: "无法运行",
    activate: "启用",
    activate_named: "启用 {version}",
    remove: "移除",
    remove_named: "移除 {version}",
    rollback: "回退",
    rollback_to: "回退到 {version}",
    switch_note: "切换版本会先停止正在运行的网络和托管的控制平面，再用新版本重新启动它们。",
    switching: "正在切换…",
    directory_label: "目录",

    releases_label: "可用版本",
    releases_intro: "版本来自 EasyTier 的 GitHub 发布页，通过 HTTPS 下载。解压前会用发布页提供的 sha256 摘要校验，\
                     未经你确认不会安装任何内容。",
    releases_empty: "GitHub 上没有可列出的版本。",
    prerelease: "预发布",
    installed_badge: "已安装",
    verified: "已用 sha256 摘要校验",
    install: "安装",
    install_named: "安装 {version}",
    blocker_too_old: "低于本应用支持的最低版本",
    blocker_no_download: "没有适用于此设备的下载",
    blocker_no_digest: "未提供 sha256 摘要，无法校验",
    phase_starting: "正在准备…",
    phase_downloading: "正在下载…",
    phase_verifying: "正在校验 sha256 摘要…",
    phase_unpacking: "正在解压…",
    installed_ready: "已安装，可以启用。",

    confirm_install_title: "安装 EasyTier {version}？",
    confirm_install_body: "将从 GitHub 下载约 {size}，解压前先校验 sha256 摘要。它会与已有版本并存，\
                           在你选择启用之前不会生效。",
    confirm_remove_title: "移除 EasyTier {version}？",
    confirm_remove_body: "它的文件会从此设备删除，其他版本不受影响。",
    confirm_adopt_title: "使用 EasyTier {version}？",
    confirm_adopt_body: "位置：{path}。正在运行的网络会用它重新启动。文件留在原处，应用不会更新或删除它们。",
    confirm_adopt_action: "使用此版本",

    path_label: "在 PATH 中找到",
    path_searching: "正在 PATH 中查找 easytier-core…",
    path_none: "PATH 中没有找到 easytier-core。",
    adopt: "使用此版本…",
    adopt_named: "使用版本 {version}",
    adopt_too_old: "低于本应用支持的最低版本",
    custom_label: "你自己安装的版本",
    custom_help: "输入同时包含 easytier-core、easytier-cli 和 easytier-web 的目录的绝对路径。",
    custom_field: "目录",
    custom_check: "检查目录",
    custom_checking: "正在检查…",

    embedded_label: "内核库版本",
    embedded_note: "EasyTier 已内置在本应用中，随应用一起更新。",
    web_note: "此服务器上的 EasyTier 程序通过部署文件更新。",
    web_action: "打开部署",

    problem_offline: "无法连接 GitHub，请检查网络后重试。",
    problem_rate_limited: "已达到 GitHub 的访问频率限制，请几分钟后重试。",
    problem_http: "GitHub 返回了意外的状态。",
    problem_bad_response: "无法读取 GitHub 的返回内容。",
    problem_no_download: "此版本没有适用于本设备的下载。",
    problem_missing_digest: "此版本没有提供 sha256 摘要，无法校验，因此不会安装。",
    problem_too_large: "下载内容超过应用允许的大小。",
    problem_size_mismatch: "下载大小与 GitHub 列出的不一致，未安装任何内容。",
    problem_digest_mismatch: "下载内容与发布页的 sha256 摘要不符，未安装任何内容。",
    problem_unsupported: "该版本低于本应用支持的最低版本。",
    problem_bad_archive: "下载的压缩包已损坏，未安装任何内容。",
    problem_unsafe_archive: "下载的压缩包含有不允许的文件，未安装任何内容。",
    problem_missing_program: "该版本缺少一个程序。",
    problem_version_unreadable: "有程序没有报告版本号。",
    problem_version_mismatch: "下载的程序不是所请求的版本，未安装任何内容。",
    problem_already_installed: "该版本已经安装。",
    problem_not_installed: "该版本尚未安装。",
    problem_active_in_use: "无法移除正在使用的版本，请先启用其他版本。",
    problem_no_previous: "没有可以回退的上一个版本。",
    problem_not_on_path: "PATH 中没有找到 easytier-core。",
    problem_not_absolute: "请输入绝对路径的目录。",
    problem_disk: "无法读取或写入此设备上的文件。",
    problem_data_dir: "无法使用应用的数据文件夹。",
    problem_interrupted: "任务意外结束，没有得到结果，请重试。",
    problem_unavailable: "此平台不支持该功能。",
};
