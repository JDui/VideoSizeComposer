//! Find complete local runtimes, then acquire official SDK ZIPs when available.
use super::*;

const DOWNLOAD_PAGE: &str = "https://www.insta360.com/sdk/record";
const RELEASE_API: &str =
    "https://api.github.com/repos/Insta360Develop/Desktop-MediaSDK-Cpp/releases/latest";
static SETUP_BUSY: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SdkSetupResult {
    state: String,
    path: Option<String>,
    message: String,
}

fn result(state: &str, sdk: Option<&Path>, message: impl Into<String>) -> SdkSetupResult {
    SdkSetupResult {
        state: state.into(),
        path: sdk
            .and_then(Path::parent)
            .map(|p| p.to_string_lossy().into()),
        message: message.into(),
    }
}

fn ready_sdk(path: &Path) -> Option<PathBuf> {
    let sdk = insta360::sdk_in_directory(path)?;
    let models = sdk.parent()?.join("models");
    fs::read_dir(models).ok()?.next()?.ok()?;
    Some(sdk)
}

pub(super) fn managed_sdk(exe_dir: &Path) -> Option<PathBuf> {
    let mut directories: Vec<_> = fs::read_dir(exe_dir.join("insta360"))
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with("sdk-"))
        .collect();
    directories.sort_by_key(|e| std::cmp::Reverse(e.metadata().and_then(|m| m.modified()).ok()));
    directories
        .into_iter()
        .find_map(|e| find_sdk_tree(&e.path()))
}

fn find_sdk_tree(root: &Path) -> Option<PathBuf> {
    ready_sdk(root).or_else(|| {
        WalkDir::new(root)
            .follow_links(false)
            .max_depth(7)
            .into_iter()
            .take(12_000)
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .find_map(|e| ready_sdk(e.path()))
    })
}

#[tauri::command]
pub(super) async fn detect_insta360_sdk(configured_path: String) -> Result<SdkSetupResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if cfg!(target_os = "macos") {
            return result(
                "unsupported",
                None,
                "官方 Desktop Media SDK 暂无 macOS 版本，请先用 Studio 导出全景 MP4。",
            );
        }
        match insta360::resolve_sdk_path(&configured_path) {
            Some(path) if ready_sdk(&path).is_some() => result(
                "ready",
                Some(&path),
                "已找到 SDK 程序及模型，编码前会检查运行依赖。",
            ),
            Some(_) => result(
                "missing",
                None,
                "SDK 不完整：缺少配套 models。请选择完整 SDK，或自动查找并下载。",
            ),
            None => result(
                "missing",
                None,
                "未找到 Desktop Media SDK，INSV 拼接需要先配置。",
            ),
        }
    })
    .await
    .map_err(|e| format!("SDK 检测失败：{e}"))
}

fn local_roots(configured: &str, exe_dir: &Path) -> Vec<PathBuf> {
    let mut roots = vec![exe_dir.join("insta360")];
    if !configured.trim().is_empty() {
        roots.push(PathBuf::from(configured.trim()));
    }
    if let Some(root) = std::env::var_os("INSTA360_MEDIA_SDK_DIR") {
        roots.push(root.into());
    }
    roots.extend(
        [
            dirs::download_dir(),
            dirs::desktop_dir(),
            dirs::document_dir(),
        ]
        .into_iter()
        .flatten(),
    );
    roots
}

fn sdk_named(path: &Path) -> bool {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    name.contains("sdk") || name.contains("insta360")
}

fn search_local(roots: &[PathBuf]) -> (Option<PathBuf>, Vec<PathBuf>) {
    let mut archives = Vec::new();
    for root in roots {
        if let Some(sdk) = ready_sdk(root) {
            return (Some(sdk), archives);
        }
        let Ok(entries) = fs::read_dir(root) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok).take(3000) {
            let path = entry.path();
            if !sdk_named(&path) {
                continue;
            }
            if path.is_dir() {
                if let Some(sdk) = find_sdk_tree(&path) {
                    return (Some(sdk), archives);
                }
            } else if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
            {
                archives.push(path);
            }
        }
    }
    archives.sort_by_key(|p| std::cmp::Reverse(p.metadata().and_then(|m| m.modified()).ok()));
    (None, archives)
}

fn official_download_url(input: &str) -> Result<url::Url, String> {
    let url = url::Url::parse(input.trim()).map_err(|_| "请输入有效的官方 HTTPS 下载直链")?;
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let official = host == "insta360.com"
        || host.ends_with(".insta360.com")
        || (host == "github.com"
            && url
                .path()
                .to_ascii_lowercase()
                .starts_with("/insta360develop/desktop-mediasdk-cpp/releases/download/"));
    if url.scheme() != "https"
        || !official
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port_or_known_default() != Some(443)
    {
        return Err("仅支持 Insta360 官方域名或官方 MediaSDK 仓库的 HTTPS 下载直链".into());
    }
    Ok(url)
}

fn release_download_url(json: &Value) -> Option<String> {
    let platform = if cfg!(windows) { "win" } else { "linux" };
    json["assets"].as_array()?.iter().find_map(|asset| {
        let name = asset["name"].as_str()?.to_ascii_lowercase();
        if !name.ends_with(".zip")
            || !name.contains(platform)
            || name.contains("darwin")
            || !(name.contains("sdk") || name.contains("media"))
        {
            return None;
        }
        let link = asset["browser_download_url"].as_str()?;
        official_download_url(link).ok().map(|url| url.to_string())
    })
}

fn curl(url: &str, output: Option<&Path>) -> Result<Vec<u8>, String> {
    let mut command =
        command_with_hidden_window(if cfg!(windows) { "curl.exe" } else { "curl" }.into());
    command.args([
        "--fail",
        "--location",
        "--silent",
        "--show-error",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
        "--connect-timeout",
        "15",
        "--user-agent",
        "VideoSizeComposer-SDK-Setup",
    ]);
    if let Some(path) = output {
        command.args(["--max-time", "900", "--output"]).arg(path);
    } else {
        command.args(["--max-time", "45", "--max-filesize", "2097152"]);
    }
    let response = command
        .arg(url)
        .output()
        .map_err(|e| format!("无法调用系统下载工具 curl：{e}"))?;
    if !response.status.success() {
        // Do not echo signed download URLs or authentication query strings.
        return Err(format!(
            "官方下载请求失败（状态 {}），可重试或前往官方页面下载 ZIP。",
            response.status
        ));
    }
    Ok(response.stdout)
}

fn archive_has_runtime(path: &Path) -> bool {
    let Ok(file) = fs::File::open(path) else {
        return false;
    };
    let Ok(mut archive) = zip::ZipArchive::new(file) else {
        return false;
    };
    let name = if cfg!(windows) {
        "MediaSDKTest.exe"
    } else {
        "MediaSDKTest"
    };
    (0..archive.len()).any(|i| {
        archive.by_index(i).ok().is_some_and(|e| {
            e.enclosed_name()
                .is_some_and(|p| p.file_name().is_some_and(|n| n.eq_ignore_ascii_case(name)))
        })
    })
}

fn extract_zip(path: &Path, destination: &Path) -> Result<(), String> {
    let file = fs::File::open(path).map_err(|e| format!("无法读取 SDK ZIP：{e}"))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|_| "下载内容不是有效的 SDK ZIP，登录页面不能用作下载直链")?;
    let mut total = 0u64;
    if archive.len() > 60_000 {
        return Err("SDK ZIP 文件项过多".into());
    }
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("SDK ZIP 已损坏：{e}"))?;
        let relative = entry.enclosed_name().ok_or("SDK ZIP 包含越界路径")?;
        if entry.name().contains(':')
            || entry.is_symlink()
            || entry.name().split(['/', '\\']).any(|part| part == "..")
        {
            return Err("SDK ZIP 包含不支持的路径或符号链接".into());
        }
        total = total.saturating_add(entry.size());
        if total > 16 * 1024 * 1024 * 1024 {
            return Err("SDK ZIP 解压大小超出 16 GB".into());
        }
        let target = destination.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|e| format!("无法写入 SDK 文件：{e}"))?;
        io::copy(&mut entry, &mut file).map_err(|e| format!("SDK 解压失败：{e}"))?;
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&target, fs::Permissions::from_mode(mode & 0o777))
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn install_archive(archive: &Path, exe_dir: &Path) -> Result<PathBuf, String> {
    let root = exe_dir.join("insta360");
    fs::create_dir_all(&root).map_err(|e| format!("无法创建程序旁的 insta360 目录：{e}"))?;
    let resolved_root = root.canonicalize().map_err(|e| e.to_string())?;
    let staging = root.join(format!(".install-{}", Uuid::new_v4()));
    fs::create_dir(&staging).map_err(|e| e.to_string())?;
    let install = (|| {
        extract_zip(archive, &staging)?;
        let sdk = find_sdk_tree(&staging)
            .ok_or("ZIP 中未找到完整 MediaSDKTest 与 models，请下载官方完整 SDK 包")?;
        let relative = sdk
            .strip_prefix(&staging)
            .map_err(|e| e.to_string())?
            .to_path_buf();
        let installed = root.join(format!("sdk-{}", Uuid::new_v4()));
        fs::rename(&staging, &installed).map_err(|e| format!("SDK 安装失败：{e}"))?;
        Ok(installed.join(relative))
    })();
    // Delete only the freshly created, resolved child of the managed SDK root.
    if let Ok(resolved) = staging.canonicalize() {
        if resolved.parent() == Some(resolved_root.as_path())
            && !fs::symlink_metadata(&staging).is_ok_and(|m| m.file_type().is_symlink())
        {
            let _ = fs::remove_dir_all(&resolved);
        }
    }
    install
}

#[tauri::command]
pub(super) async fn setup_insta360_sdk(
    app: tauri::AppHandle,
    configured_path: String,
    download_url: Option<String>,
) -> Result<SdkSetupResult, String> {
    if SETUP_BUSY
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("SDK 查找或下载正在进行，请稍候".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        struct BusyGuard;
        impl Drop for BusyGuard { fn drop(&mut self) { SETUP_BUSY.store(false, Ordering::SeqCst); } }
        let _busy = BusyGuard;
        if cfg!(target_os = "macos") { return Ok(result("unsupported", None, "官方 Desktop Media SDK 暂无 macOS 版本，请先用 Studio 导出全景 MP4。")); }
        let exe_dir = executable_dir_path()?;
        let explicit_url = download_url.filter(|u| !u.trim().is_empty());
        if explicit_url.is_none() {
            let _ = app.emit("insta360-sdk-progress", "正在搜索程序目录、下载、桌面与文档中的 SDK…");
            if let Some(sdk) = insta360::resolve_sdk_path(&configured_path).and_then(|p| ready_sdk(&p))
                .or_else(|| insta360::resolve_sdk_path("").and_then(|p| ready_sdk(&p)))
                .or_else(|| (!configured_path.trim().is_empty()).then(|| find_sdk_tree(Path::new(configured_path.trim()))).flatten()) {
                return Ok(result("ready", Some(&sdk), "已找到 SDK，保存设置后用于 INSV 拼接。"));
            }
            let (sdk, archives) = search_local(&local_roots(&configured_path, &exe_dir));
            if let Some(sdk) = sdk { return Ok(result("ready", Some(&sdk), "已找到本机 SDK，保存设置后用于 INSV 拼接。")); }
            for archive in archives.into_iter().take(8) {
                if !archive_has_runtime(&archive) { continue; }
                let _ = app.emit("insta360-sdk-progress", "找到本机 SDK ZIP，正在解压配置…");
                let sdk = install_archive(&archive, &exe_dir)?;
                return Ok(result("ready", Some(&sdk), "已从本机 ZIP 安装 SDK，保存设置即可使用。"));
            }
        }
        let url = if let Some(link) = explicit_url { official_download_url(&link)?.to_string() } else {
            let _ = app.emit("insta360-sdk-progress", "正在检查 Insta360 官方 SDK 下载…");
            let release: Value = serde_json::from_slice(&curl(RELEASE_API, None)?).map_err(|_| "官方下载信息无法读取")?;
            let Some(link) = release_download_url(&release) else {
                return Ok(result("authorization_required", None, "官方当前未公开完整 SDK 下载包，需登录并申请。下载 ZIP 到“下载”文件夹后，再点自动查找；也可填入官方 ZIP 直链。"));
            };
            link
        };
        let archive = std::env::temp_dir().join(format!("vsc-sdk-{}.zip", Uuid::new_v4()));
        let _archive_guard = TemporaryFileGuard(archive.clone());
        let _ = app.emit("insta360-sdk-progress", "正在下载官方 SDK ZIP，请稍候…");
        curl(&url, Some(&archive))?;
        let _ = app.emit("insta360-sdk-progress", "下载完成，正在解压并检测 SDK…");
        let sdk = install_archive(&archive, &exe_dir)?;
        Ok(result("ready", Some(&sdk), "SDK 已下载并安装，保存设置即可使用。"))
    }).await.map_err(|e| format!("SDK 配置任务失败：{e}"))?
}

#[tauri::command]
pub(super) fn open_insta360_sdk_download() -> Result<(), String> {
    let mut command = if cfg!(windows) {
        let mut command = command_with_hidden_window("rundll32.exe".into());
        command.arg("url.dll,FileProtocolHandler");
        command
    } else {
        command_with_hidden_window(
            if cfg!(target_os = "macos") {
                "open"
            } else {
                "xdg-open"
            }
            .into(),
        )
    };
    command
        .arg(DOWNLOAD_PAGE)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("无法打开官方下载页面：{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v12_05_sdk_discovery_and_zip_install() {
        for url in [
            "http://res.insta360.com/sdk.zip",
            "https://insta360.com.evil.test/sdk.zip",
            "https://insta360.com@evil.test/sdk.zip",
            "https://github.com/other/repo/releases/download/v1/sdk.zip",
        ] {
            assert!(official_download_url(url).is_err());
        }
        assert!(official_download_url("https://res.insta360.com/sdk.zip?token=private").is_ok());
        assert!(
            release_download_url(&serde_json::json!({"assets":[]})).is_none(),
            "source-only releases cannot be reported as downloadable SDKs"
        );
        let platform = if cfg!(windows) { "Windows" } else { "Linux" };
        let release = serde_json::json!({"assets":[{"name":format!("{platform}-MediaSDK.zip"),"browser_download_url":"https://github.com/Insta360Develop/Desktop-MediaSDK-Cpp/releases/download/v1/sdk.zip"}]});
        assert!(release_download_url(&release).is_some());

        struct TestRoot(PathBuf);
        impl Drop for TestRoot {
            fn drop(&mut self) {
                let parent = Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("target")
                    .canonicalize()
                    .unwrap();
                if let Ok(path) = self.0.canonicalize() {
                    if path.parent() == Some(parent.as_path()) {
                        let _ = fs::remove_dir_all(path);
                    }
                }
            }
        }
        let root = TestRoot(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("target")
                .join(format!("vsc-sdk-test-{}", Uuid::new_v4())),
        );
        fs::create_dir_all(&root.0).unwrap();
        let archive_path = root.0.join("Insta360-MediaSDK.zip");
        let mut writer = zip::ZipWriter::new(fs::File::create(&archive_path).unwrap());
        let executable = if cfg!(windows) {
            "MediaSDKTest.exe"
        } else {
            "MediaSDKTest"
        };
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        writer
            .start_file(format!("官方 SDK/bin/{executable}"), options)
            .unwrap();
        writer.write_all(b"fixture; never executed").unwrap();
        writer
            .start_file("官方 SDK/bin/models/model.ins", options)
            .unwrap();
        writer.write_all(b"fixture model").unwrap();
        writer.finish().unwrap();
        assert!(archive_has_runtime(&archive_path));
        let sdk = install_archive(&archive_path, &root.0).unwrap();
        assert!(ready_sdk(&sdk).is_some());
        assert_eq!(managed_sdk(&root.0), Some(sdk.clone()));
        let (found, _) = search_local(&[root.0.join("insta360")]);
        assert_eq!(found, Some(sdk));
        let incomplete_path = root.0.join("incomplete.zip");
        let mut incomplete = zip::ZipWriter::new(fs::File::create(&incomplete_path).unwrap());
        incomplete.start_file(format!("bin/{executable}"), options).unwrap();
        incomplete.write_all(b"missing models; never executed").unwrap();
        incomplete.finish().unwrap();
        assert!(install_archive(&incomplete_path, &root.0).unwrap_err().contains("完整"));
        assert!(managed_sdk(&root.0).is_some(), "an invalid package must leave the installed SDK intact");
        let mut malicious = zip::ZipWriter::new(fs::File::create(root.0.join("bad.zip")).unwrap());
        malicious.start_file("../escaped.txt", options).unwrap();
        malicious.write_all(b"not allowed").unwrap();
        malicious.finish().unwrap();
        let target = root.0.join("extract");
        fs::create_dir(&target).unwrap();
        assert!(extract_zip(&root.0.join("bad.zip"), &target).is_err());
        assert!(!root.0.join("escaped.txt").exists());
    }
}
