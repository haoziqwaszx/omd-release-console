use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

const DEFAULT_RELEASE_CONFIG: &str = include_str!("../release.config.json");

const STEP_KEYS: [&str; 13] = [
    "preflight",
    "buildWindows",
    "collectWindowsArtifacts",
    "buildMac",
    "artifactCheck",
    "checksums",
    "manifestGithub",
    "manifestGitee",
    "dryRunRelease",
    "verify",
    "publishGithub",
    "publishGitee",
    "report",
];

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReleaseConfig {
    source_repo: String,
    github_repo: String,
    gitee_owner: String,
    gitee_repo: String,
    gitee_latest: String,
    github_latest: String,
    windows_ssh: String,
    windows_repo: String,
    windows_target: String,
    windows_build_script: Option<String>,
    windows_build_dir: Option<String>,
    windows_export_dir_pattern: Option<String>,
    windows_branch: Option<String>,
    github_proxy: Option<String>,
    release_dir_pattern: String,
    required_artifacts: Vec<RequiredArtifact>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequiredArtifact {
    key: String,
    scope: String,
    file: String,
    signature: String,
    source_file: Option<String>,
    source_signature: Option<String>,
}

#[derive(Clone)]
struct AppContext {
    root: PathBuf,
    public_dir: PathBuf,
    desktop_dir: PathBuf,
    data_dir: PathBuf,
    config: ReleaseConfig,
}

struct StateManager {
    file: PathBuf,
    release_dir: PathBuf,
    version: String,
    source_repo: String,
    commit: String,
}

struct CliContext {
    app: AppContext,
    version: String,
    release_dir: PathBuf,
    state: StateManager,
    flags: BTreeMap<String, Option<String>>,
}

struct CommandOutput {
    status: i32,
    stdout: String,
    stderr: String,
}

#[derive(Debug, PartialEq, Eq)]
struct CommandSpec {
    command: String,
    args: Vec<String>,
    cwd: Option<String>,
}

impl CommandSpec {
    fn new(command: &str, args: Vec<&str>, cwd: Option<&str>) -> Self {
        Self {
            command: command.to_string(),
            args: args.into_iter().map(ToString::to_string).collect(),
            cwd: cwd.map(ToString::to_string),
        }
    }
}

fn main() {
    if let Err(error) = run_main() {
        eprintln!("错误：{error}");
        std::process::exit(1);
    }
}

fn run_main() -> Result<(), String> {
    let app = AppContext::load()?;
    let args: Vec<String> = env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("desktop");

    match command {
        "desktop" | "app" | "gui" => run_desktop(app),
        "serve" | "server" | "dev" => serve(app),
        "plan" => {
            print_plan();
            Ok(())
        }
        "preflight"
        | "build-windows"
        | "collect-windows-artifacts"
        | "build-mac"
        | "check-artifacts"
        | "manifest"
        | "verify"
        | "report"
        | "release" => run_cli_command(app, &args),
        "--help" | "-h" | "help" => {
            print_help();
            Ok(())
        }
        other => Err(format!("未知命令：{other}")),
    }
}

impl AppContext {
    fn load() -> Result<Self, String> {
        let root = env::current_dir().map_err(|error| error.to_string())?;
        let public_dir = root.join("public");
        let desktop_dir = home_dir().join("Desktop");
        let data_dir = app_data_dir();
        let config = load_release_config(&root)?;
        Ok(Self {
            root,
            public_dir,
            desktop_dir,
            data_dir,
            config,
        })
    }
}

fn run_desktop(app: AppContext) -> Result<(), String> {
    tauri::Builder::default()
        .manage(app)
        .invoke_handler(tauri::generate_handler![
            tauri_config,
            tauri_summary,
            tauri_commands,
            tauri_action,
            tauri_action_log,
            tauri_latest_release_dir,
            tauri_open_release_dir,
            tauri_history
        ])
        .run(tauri::generate_context!())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn tauri_config(app: State<'_, AppContext>) -> Value {
    json!({ "ok": true, "config": config_summary(&app.config) })
}

#[tauri::command]
fn tauri_summary(app: State<'_, AppContext>, release_dir: Option<String>) -> Result<Value, String> {
    let release_dir = release_dir
        .filter(|value| !value.trim().is_empty())
        .map(|value| expand_home(value.trim()))
        .or_else(|| find_latest_release_dir(&app.desktop_dir));
    summarize_release(&app, release_dir.as_deref())
}

#[tauri::command]
fn tauri_commands(version: Option<String>) -> Value {
    let version = version
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("0.0.7");
    release_commands(version)
}

#[tauri::command]
async fn tauri_action(
    app: State<'_, AppContext>,
    action: String,
    version: String,
    release_dir: String,
    confirm_version: Option<String>,
) -> Result<Value, String> {
    let app = app.inner().clone();
    let body = json!({
        "version": version,
        "releaseDir": release_dir,
        "confirmVersion": confirm_version.unwrap_or_default()
    });
    tauri::async_runtime::spawn_blocking(move || run_action(&app, &action, &body))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
fn tauri_action_log(
    app: State<'_, AppContext>,
    action: String,
    version: String,
    release_dir: String,
) -> Value {
    let body = json!({
        "version": version,
        "releaseDir": release_dir,
    });
    action_log_response(&app, &action, &body)
}

#[tauri::command]
fn tauri_latest_release_dir(app: State<'_, AppContext>) -> Value {
    json!({
        "ok": true,
        "releaseDir": find_latest_release_dir(&app.desktop_dir).map(|path| path.to_string_lossy().to_string())
    })
}

#[tauri::command]
fn tauri_open_release_dir(
    app: State<'_, AppContext>,
    release_dir: String,
) -> Result<Value, String> {
    let release_dir = absolute_path(&expand_home(release_dir.trim()));
    if !release_dir.exists() || !release_dir.is_dir() {
        return Ok(action_error(
            "open-release-dir",
            "invalid_input",
            "releaseDir 不存在。",
        ));
    }
    if release_dir.strip_prefix(&app.desktop_dir).is_err() {
        return Ok(action_error(
            "open-release-dir",
            "invalid_input",
            "只能打开桌面下的 release 目录。",
        ));
    }
    run_status("open", &[release_dir.to_string_lossy().as_ref()], None)?;
    Ok(json!({ "ok": true, "releaseDir": release_dir.to_string_lossy() }))
}

#[tauri::command]
fn tauri_history(app: State<'_, AppContext>) -> Value {
    load_release_history(&app.data_dir)
}

fn load_release_config(root: &Path) -> Result<ReleaseConfig, String> {
    let config_file = root.join("release.config.json");
    let raw = if config_file.exists() {
        fs::read_to_string(&config_file)
            .map_err(|error| format!("读取 release.config.json 失败：{error}"))?
    } else {
        DEFAULT_RELEASE_CONFIG.to_string()
    };
    let mut config: ReleaseConfig = serde_json::from_str(&raw)
        .map_err(|error| format!("解析 release.config.json 失败：{error}"))?;
    if let Ok(proxy) = env::var("OMD_RELEASE_PROXY") {
        if !proxy.trim().is_empty() {
            config.github_proxy = Some(proxy);
        }
    }
    validate_config(&config)?;
    Ok(config)
}

fn validate_config(config: &ReleaseConfig) -> Result<(), String> {
    let required = [
        ("sourceRepo", &config.source_repo),
        ("githubRepo", &config.github_repo),
        ("giteeOwner", &config.gitee_owner),
        ("giteeRepo", &config.gitee_repo),
        ("giteeLatest", &config.gitee_latest),
        ("githubLatest", &config.github_latest),
        ("windowsSsh", &config.windows_ssh),
        ("windowsRepo", &config.windows_repo),
        ("windowsTarget", &config.windows_target),
        ("releaseDirPattern", &config.release_dir_pattern),
    ];

    for (key, value) in required {
        if value.trim().is_empty() {
            return Err(format!("release.config.json 缺少字段：{key}"));
        }
    }

    if let Some(value) = config.windows_build_script.as_deref() {
        ensure(!value.trim().is_empty(), "windowsBuildScript 不能为空")?;
    }
    if let Some(value) = config.windows_build_dir.as_deref() {
        ensure(!value.trim().is_empty(), "windowsBuildDir 不能为空")?;
    }
    if let Some(value) = config.windows_export_dir_pattern.as_deref() {
        ensure(
            value.contains("{version}"),
            "windowsExportDirPattern 必须包含 {version}",
        )?;
    }
    if let Some(value) = config.windows_branch.as_deref() {
        ensure(!value.trim().is_empty(), "windowsBranch 不能为空")?;
    }

    if config.required_artifacts.is_empty() {
        return Err("release.config.json 缺少 requiredArtifacts".to_string());
    }

    for artifact in &config.required_artifacts {
        if artifact.key.is_empty()
            || artifact.scope.is_empty()
            || artifact.file.is_empty()
            || artifact.signature.is_empty()
        {
            return Err("requiredArtifacts 缺少 key/scope/file/signature".to_string());
        }
        if !matches!(artifact.scope.as_str(), "windows" | "mac") {
            return Err("requiredArtifacts scope 只支持 windows/mac".to_string());
        }
    }

    Ok(())
}

fn run_cli_command(app: AppContext, args: &[String]) -> Result<(), String> {
    let command = args[0].as_str();
    let version = args
        .get(1)
        .ok_or_else(|| "请提供 SemVer 版本号，例如 0.0.7。".to_string())?
        .to_string();
    if !is_version(&version) {
        return Err("请提供 SemVer 版本号，例如 0.0.7。".to_string());
    }

    let flags = parse_flags(&args[2..]);
    let release_dir = flags
        .get("releaseDir")
        .and_then(|value| value.clone())
        .map(|value| expand_home(&value))
        .unwrap_or_else(|| {
            home_dir()
                .join("Desktop")
                .join(format!("omd-{version}-release-{}", timestamp()))
        });
    let release_dir = absolute_path(&release_dir);
    let source_repo = app.config.source_repo.clone();
    let commit = if Path::new(&source_repo).join(".git").exists() {
        run_capture("git", &["rev-parse", "HEAD"], Some(Path::new(&source_repo)))
            .unwrap_or_default()
    } else {
        String::new()
    };
    let state = StateManager::new(release_dir.clone(), version.clone(), source_repo, commit)?;

    let ctx = CliContext {
        app,
        version,
        release_dir,
        state,
        flags,
    };

    match command {
        "preflight" => preflight(&ctx),
        "build-windows" => build_windows(&ctx),
        "collect-windows-artifacts" => collect_windows_artifacts(&ctx),
        "build-mac" => build_mac(&ctx),
        "check-artifacts" => check_artifacts(&ctx),
        "manifest" => {
            checksums(&ctx)?;
            manifests(&ctx)
        }
        "verify" => verify(&ctx),
        "report" => report(&ctx),
        "release" => release(&ctx),
        _ => Err(format!("未知命令：{command}")),
    }
}

impl StateManager {
    fn new(
        release_dir: PathBuf,
        version: String,
        source_repo: String,
        commit: String,
    ) -> Result<Self, String> {
        fs::create_dir_all(&release_dir).map_err(|error| error.to_string())?;
        let file = release_dir.join("state.json");
        Ok(Self {
            file,
            release_dir,
            version,
            source_repo,
            commit,
        })
    }

    fn load_state(&self) -> Value {
        let mut state = if self.file.exists() {
            fs::read_to_string(&self.file)
                .ok()
                .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
                .unwrap_or_else(|| json!({}))
        } else {
            let now = iso_now();
            json!({
                "version": self.version,
                "releaseDir": self.release_dir.to_string_lossy(),
                "sourceRepo": self.source_repo,
                "commit": self.commit,
                "createdAt": now,
                "updatedAt": now,
                "currentStep": "",
                "overallStatus": "not_started",
                "runId": format!("{}-{}", self.version, timestamp()),
                "checks": [],
                "steps": {},
                "artifacts": [],
                "manifests": {},
                "errors": [],
                "suggestions": [],
                "history": []
            })
        };

        state["version"] = json!(state["version"].as_str().unwrap_or(&self.version));
        state["releaseDir"] = json!(state["releaseDir"]
            .as_str()
            .unwrap_or(&self.release_dir.to_string_lossy()));
        state["sourceRepo"] = json!(state["sourceRepo"].as_str().unwrap_or(&self.source_repo));
        state["commit"] = json!(state["commit"].as_str().unwrap_or(&self.commit));
        if state["runId"].as_str().unwrap_or("").is_empty() {
            state["runId"] = json!(format!("{}-{}", self.version, timestamp()));
        }
        ensure_array(&mut state, "checks");
        ensure_array(&mut state, "artifacts");
        ensure_object(&mut state, "manifests");
        ensure_array(&mut state, "errors");
        ensure_array(&mut state, "suggestions");
        ensure_array(&mut state, "history");
        ensure_object(&mut state, "steps");

        for key in STEP_KEYS {
            if state["steps"].get(key).is_none() {
                state["steps"][key] = empty_step(key);
            }
        }

        state
    }

    fn save_state(&self, mut state: Value) -> Result<Value, String> {
        state["updatedAt"] = json!(iso_now());
        let body = serde_json::to_string_pretty(&state).map_err(|error| error.to_string())?;
        fs::write(&self.file, format!("{body}\n")).map_err(|error| error.to_string())?;
        Ok(state)
    }

    fn start_step(&self, step_key: &str, message: &str) -> Result<(), String> {
        let mut state = self.load_state();
        let now = iso_now();
        state["currentStep"] = json!(step_key);
        state["overallStatus"] = json!("running");
        push_history(&mut state, "step_started", step_key, message);
        let mut step = state["steps"][step_key].clone();
        step["status"] = json!("running");
        step["startedAt"] = json!(now);
        step["endedAt"] = json!("");
        step["durationMs"] = json!(0);
        step["message"] = json!(message);
        step["error"] = Value::Null;
        state["steps"][step_key] = step;
        self.save_state(state).map(|_| ())
    }

    fn complete_step(&self, step_key: &str, message: &str, summary: Value) -> Result<(), String> {
        let mut state = self.load_state();
        let now = iso_now();
        let started_at = state["steps"][step_key]["startedAt"]
            .as_str()
            .unwrap_or("")
            .to_string();
        state["currentStep"] = json!(step_key);
        state["overallStatus"] = json!("success");
        push_history(&mut state, "step_completed", step_key, message);
        let mut step = state["steps"][step_key].clone();
        step["status"] = json!("success");
        step["endedAt"] = json!(now.clone());
        step["durationMs"] = json!(duration_ms(&started_at, &now));
        step["message"] = json!(message);
        step["summary"] = summary;
        step["error"] = Value::Null;
        state["steps"][step_key] = step;
        self.save_state(state).map(|_| ())
    }

    fn fail_step(&self, step_key: &str, message: &str, suggestion: &str) -> Result<(), String> {
        let mut state = self.load_state();
        let now = iso_now();
        let started_at = state["steps"][step_key]["startedAt"]
            .as_str()
            .unwrap_or("")
            .to_string();
        state["currentStep"] = json!(step_key);
        state["overallStatus"] = json!("failed");
        push_history(&mut state, "step_failed", step_key, message);
        push_array(
            &mut state["errors"],
            json!({ "step": step_key, "message": message, "at": now }),
        );
        if !suggestion.is_empty() {
            upsert_suggestion(&mut state, suggestion, "error", step_key);
        }
        let mut step = state["steps"][step_key].clone();
        step["status"] = json!("failed");
        step["endedAt"] = json!(now.clone());
        step["durationMs"] = json!(duration_ms(&started_at, &now));
        step["error"] = json!(message);
        if !suggestion.is_empty() {
            step["suggestions"] = json!([suggestion]);
        }
        state["steps"][step_key] = step;
        self.save_state(state).map(|_| ())
    }

    fn record_check(&self, check: Value) -> Result<(), String> {
        let mut state = self.load_state();
        upsert_by_key(&mut state["checks"], with_default_time(check, "checkedAt"));
        self.save_state(state).map(|_| ())
    }

    fn record_artifacts(&self, artifacts: Vec<Value>) -> Result<(), String> {
        let mut state = self.load_state();
        state["artifacts"] = Value::Array(artifacts);
        self.save_state(state).map(|_| ())
    }

    fn record_manifest(&self, host: &str, manifest: Value) -> Result<(), String> {
        let mut state = self.load_state();
        ensure_object(&mut state, "manifests");
        state["manifests"][host] = manifest;
        self.save_state(state).map(|_| ())
    }

    fn record_suggestion(&self, message: &str, severity: &str, action: &str) -> Result<(), String> {
        let mut state = self.load_state();
        upsert_suggestion(&mut state, message, severity, action);
        self.save_state(state).map(|_| ())
    }

    fn update_commit(&self, commit: &str) -> Result<(), String> {
        let mut state = self.load_state();
        state["commit"] = json!(commit);
        self.save_state(state).map(|_| ())
    }
}

fn preflight(ctx: &CliContext) -> Result<(), String> {
    ctx.state.start_step("preflight", "开始预检")?;
    progress("开始预检");
    let mut failures: Vec<(String, String)> = Vec::new();

    check(
        ctx,
        &mut failures,
        "sourcePackageVersion",
        "源码 package.json 版本",
        "请先同步 package.json version",
        || {
            let package_json =
                read_json(&Path::new(&ctx.app.config.source_repo).join("package.json"))?;
            let current = package_json["version"].as_str().unwrap_or("");
            ensure(
                current == ctx.version,
                &format!("package.json 当前版本是 {current}"),
            )?;
            Ok(format!("package.json 版本 {current}"))
        },
    )?;

    let tauri_config = match read_json(
        &Path::new(&ctx.app.config.source_repo).join("src-tauri/tauri.conf.json"),
    ) {
        Ok(value) => value,
        Err(error) => {
            failures.push((
                "tauriVersion".to_string(),
                "请先同步 src-tauri/tauri.conf.json version".to_string(),
            ));
            ctx.state.record_check(json!({
                "key": "tauriVersion",
                "label": "Tauri 配置版本",
                "status": "failed",
                "message": error,
                "suggestion": "请先同步 src-tauri/tauri.conf.json version"
            }))?;
            Value::Null
        }
    };

    if !tauri_config.is_null() {
        check(
            ctx,
            &mut failures,
            "tauriVersion",
            "Tauri 配置版本",
            "请先同步 src-tauri/tauri.conf.json version",
            || {
                let current = tauri_config["version"].as_str().unwrap_or("");
                ensure(
                    current == ctx.version,
                    &format!("tauri.conf.json 当前版本是 {current}"),
                )?;
                Ok(format!("tauri.conf.json 版本 {current}"))
            },
        )?;
    }

    check(
        ctx,
        &mut failures,
        "updaterKey",
        "Updater 私钥",
        "请确认 src-tauri/updater.key 是否在本机存在",
        || {
            ensure(
                Path::new(&ctx.app.config.source_repo)
                    .join("src-tauri/updater.key")
                    .exists(),
                "缺少 src-tauri/updater.key",
            )?;
            Ok("updater.key 存在".to_string())
        },
    )?;

    check(
        ctx,
        &mut failures,
        "ghCli",
        "gh CLI",
        "请安装 gh CLI 或确认 PATH",
        || {
            ensure(has_command("gh"), "缺少 gh CLI")?;
            Ok("gh CLI 存在".to_string())
        },
    )?;

    check(
        ctx,
        &mut failures,
        "sshCli",
        "ssh CLI",
        "请安装 ssh 或确认 PATH",
        || {
            ensure(has_command("ssh"), "缺少 ssh")?;
            Ok("ssh 存在".to_string())
        },
    )?;

    check(
        ctx,
        &mut failures,
        "scpCli",
        "scp CLI",
        "请安装 scp 或确认 PATH",
        || {
            ensure(has_command("scp"), "缺少 scp")?;
            Ok("scp 存在".to_string())
        },
    )?;

    check(
        ctx,
        &mut failures,
        "githubAuth",
        "GitHub 登录状态",
        "请运行 gh auth login",
        || {
            run_status(
                "gh",
                &["auth", "status"],
                Some(Path::new(&ctx.app.config.source_repo)),
            )?;
            Ok("gh auth status 通过".to_string())
        },
    )?;

    check(
        ctx,
        &mut failures,
        "windowsSsh",
        "Windows 构建机 SSH",
        "请检查 4090 SSH 连通性和密钥配置",
        || {
            run_status(
                "ssh",
                &[
                    "-o",
                    "StrictHostKeyChecking=accept-new",
                    "-o",
                    "BatchMode=yes",
                    &ctx.app.config.windows_ssh,
                    "powershell -NoProfile -Command \"hostname; whoami\"",
                ],
                Some(Path::new(&ctx.app.config.source_repo)),
            )?;
            Ok("Windows SSH 连接成功".to_string())
        },
    )?;

    check(
        ctx,
        &mut failures,
        "giteeCredential",
        "Gitee 凭据",
        "请检查 git credential 中的 Gitee 凭据",
        || {
            ensure(has_gitee_credential(), "无法读取 Gitee 凭据")?;
            Ok("Gitee 凭据可用".to_string())
        },
    )?;

    check(
        ctx,
        &mut failures,
        "giteeEndpoint",
        "Gitee updater 端点",
        "请检查 tauri updater endpoints",
        || {
            let endpoints = updater_endpoints(&tauri_config);
            ensure(
                endpoints
                    .iter()
                    .any(|endpoint| endpoint == &ctx.app.config.gitee_latest),
                "缺少 Gitee updater 端点",
            )?;
            Ok("Gitee updater 端点存在".to_string())
        },
    )?;

    check(
        ctx,
        &mut failures,
        "githubEndpoint",
        "GitHub updater 端点",
        "请检查 tauri updater endpoints",
        || {
            let endpoints = updater_endpoints(&tauri_config);
            ensure(
                endpoints
                    .iter()
                    .any(|endpoint| endpoint == &ctx.app.config.github_latest),
                "缺少 GitHub updater 端点",
            )?;
            Ok("GitHub updater 端点存在".to_string())
        },
    )?;

    if !failures.is_empty() {
        let message = format!("预检失败 {} 项", failures.len());
        let suggestion = failures[0].1.clone();
        ctx.state.fail_step("preflight", &message, &suggestion)?;
        return Err(message);
    }

    let checks_len = ctx.state.load_state()["checks"]
        .as_array()
        .map(|items| items.len())
        .unwrap_or(0);
    ctx.state
        .complete_step("preflight", "预检通过", json!({ "checks": checks_len }))?;
    progress("预检通过");
    Ok(())
}

fn check<F>(
    ctx: &CliContext,
    failures: &mut Vec<(String, String)>,
    key: &str,
    label: &str,
    suggestion: &str,
    fn_check: F,
) -> Result<(), String>
where
    F: FnOnce() -> Result<String, String>,
{
    match fn_check() {
        Ok(message) => ctx.state.record_check(json!({
            "key": key,
            "label": label,
            "status": "success",
            "message": message,
            "suggestion": ""
        })),
        Err(error) => {
            failures.push((key.to_string(), suggestion.to_string()));
            ctx.state.record_check(json!({
                "key": key,
                "label": label,
                "status": "failed",
                "message": error,
                "suggestion": suggestion
            }))?;
            ctx.state.record_suggestion(suggestion, "error", key)
        }
    }
}

fn release(ctx: &CliContext) -> Result<(), String> {
    if let Some(step) = ctx.flags.get("step").and_then(|value| value.as_deref()) {
        return publish_step(ctx, step);
    }

    if let Some(resume_from) = ctx
        .flags
        .get("resumeFrom")
        .and_then(|value| value.as_deref())
    {
        return run_safe_release_sequence(ctx, resume_from);
    }

    fs::create_dir_all(&ctx.release_dir).map_err(|error| error.to_string())?;
    progress(&format!("发布目录：{}", ctx.release_dir.display()));

    if ctx.flags.contains_key("dryRun") {
        preflight(ctx)?;
        ctx.state
            .start_step("dryRunRelease", "生成 dry-run 发布命令")?;
        let commands = dry_run_commands(ctx)?;
        for command in &commands {
            println!("[试运行] {command}");
        }
        ctx.state.complete_step(
            "dryRunRelease",
            "dry-run 发布命令已生成",
            json!({ "commands": commands }),
        )?;
        return Ok(());
    }

    if !confirm_version_matches(&ctx.flags, &ctx.version) {
        ctx.state.fail_step(
            "publishGithub",
            "版本确认不匹配",
            "运行完整发布必须传入 --confirm-version 且值与目标版本完全一致",
        )?;
        return Err("运行完整发布必须传入 --confirm-version 且值与目标版本完全一致。".to_string());
    }

    run_safe_release_sequence(ctx, "preflight")
}

fn confirm_version_matches(flags: &BTreeMap<String, Option<String>>, version: &str) -> bool {
    flags
        .get("confirmVersion")
        .and_then(|value| value.as_deref())
        .is_some_and(|confirm| confirm == version)
}

fn run_safe_release_sequence(ctx: &CliContext, resume_from: &str) -> Result<(), String> {
    let mut active = false;
    for step in release_step_order() {
        if step == resume_from {
            active = true;
        }
        if !active {
            continue;
        }
        match step {
            "preflight" => preflight(ctx)?,
            "build-windows" => build_windows(ctx)?,
            "build-mac" => build_mac(ctx)?,
            "collect-windows-artifacts" => collect_windows_artifacts(ctx)?,
            "check-artifacts" => check_artifacts(ctx)?,
            "checksums" => checksums(ctx)?,
            "manifest" => manifests(ctx)?,
            "publish-github" => publish_step(ctx, "publish-github")?,
            "publish-gitee" => publish_step(ctx, "publish-gitee")?,
            "verify" => verify(ctx)?,
            "report" => report(ctx)?,
            _ => return Err(format!("未知恢复步骤：{step}")),
        }
    }
    if !active {
        return Err(format!("未知恢复起点：{resume_from}"));
    }
    Ok(())
}

fn release_step_order() -> Vec<&'static str> {
    vec![
        "preflight",
        "build-windows",
        "build-mac",
        "collect-windows-artifacts",
        "check-artifacts",
        "checksums",
        "manifest",
        "publish-github",
        "publish-gitee",
        "verify",
        "report",
    ]
}

fn upload_windows_build_script(ctx: &CliContext) -> Result<(), String> {
    let local_script = local_windows_build_script(&ctx.app.root);
    if !local_script.exists() {
        return Err(format!("缺少 Windows 构建脚本：{}", local_script.display()));
    }
    let remote_script = windows_build_script(&ctx.app.config);
    let remote_arg = format!(
        "{}:{}",
        ctx.app.config.windows_ssh,
        windows_scp_path(&remote_script)
    );
    let local_arg = local_script.to_string_lossy().to_string();
    let output = run_command_output(
        "scp",
        &["-q", local_arg.as_str(), remote_arg.as_str()],
        Some(Path::new(&ctx.app.config.source_repo)),
    )?;
    if output.status != 0 {
        return Err(command_output_summary(&output));
    }
    Ok(())
}

fn run_remote_powershell(ctx: &CliContext, script: &str) -> Result<CommandOutput, String> {
    let script = format!(
        "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; $OutputEncoding = [System.Text.Encoding]::UTF8; $ProgressPreference = 'SilentlyContinue'; {script}\n"
    );
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    let script_name = format!("omd-release-console-{stamp}-{}.ps1", std::process::id());
    let local_script = env::temp_dir().join(&script_name);
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(script.as_bytes());
    fs::write(&local_script, bytes).map_err(|error| error.to_string())?;

    let remote_script = format!("C:/Users/4090/AppData/Local/Temp/{script_name}");
    let remote_arg = format!(
        "{}:/C:/Users/4090/AppData/Local/Temp/{script_name}",
        ctx.app.config.windows_ssh
    );
    let local_arg = local_script.to_string_lossy().to_string();
    let copy_output = run_command_output(
        "scp",
        &["-q", local_arg.as_str(), remote_arg.as_str()],
        Some(Path::new(&ctx.app.config.source_repo)),
    );
    let _ = fs::remove_file(&local_script);
    let copy_output = copy_output?;
    if copy_output.status != 0 {
        return Ok(copy_output);
    }

    let args = vec![
        ctx.app.config.windows_ssh.as_str(),
        "powershell",
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        remote_script.as_str(),
    ];
    run_command_output("ssh", &args, Some(Path::new(&ctx.app.config.source_repo)))
}

fn is_windows_rustc_access_violation(output: &CommandOutput) -> bool {
    let combined = format!("{}\n{}", output.stdout, output.stderr).to_ascii_lowercase();
    combined.contains("status_access_violation") || combined.contains("0xc0000005")
}

fn remote_commit_from_output(output: &CommandOutput) -> Option<String> {
    output.stdout.lines().find_map(|line| {
        line.trim()
            .strip_prefix("OMD_REMOTE_COMMIT=")
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string)
    })
}

fn mac_build_command_sequence(source_repo: &str) -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("git", vec!["pull", "--ff-only"], Some(source_repo)),
        CommandSpec::new(
            "npm",
            vec!["--prefix", source_repo, "run", "tauri:build:mac"],
            None,
        ),
    ]
}

fn run_command_spec(spec: &CommandSpec) -> Result<CommandOutput, String> {
    let args = spec.args.iter().map(String::as_str).collect::<Vec<_>>();
    let cwd = spec.cwd.as_deref().map(Path::new);
    run_command_output(&spec.command, &args, cwd)
}

fn render_artifact_source(template: &str, version: &str, windows_target: &str) -> String {
    template
        .replace("{version}", version)
        .replace("{windowsTarget}", windows_target)
}

fn build_windows(ctx: &CliContext) -> Result<(), String> {
    ctx.state
        .start_step("buildWindows", "开始 4090 Windows 打包")?;
    let export_dir = windows_export_dir(&ctx.app.config, &ctx.version);
    upload_windows_build_script(ctx).inspect_err(|_| {
        let _ = ctx.state.fail_step(
            "buildWindows",
            "Windows 构建脚本上传失败",
            "检查 4090 scp 权限和 scripts/windows/omd-release-build.ps1 是否存在",
        );
    })?;
    let remote_script = windows_build_script_invocation(&ctx.app.config, &ctx.version, false);
    let output = run_remote_powershell(ctx, &remote_script)?;
    let retry_output = if output.status != 0 && is_windows_rustc_access_violation(&output) {
        progress("检测到 Windows rustc 访问冲突，清理 Rust target 后自动重试一次");
        let retry_script = windows_build_script_invocation(&ctx.app.config, &ctx.version, true);
        Some(run_remote_powershell(ctx, &retry_script)?)
    } else {
        None
    };
    let final_output = retry_output.as_ref().unwrap_or(&output);
    if final_output.status == 0 {
        let remote_commit = remote_commit_from_output(final_output).unwrap_or_default();
        if !remote_commit.is_empty() {
            ctx.state.update_commit(&remote_commit)?;
        }
        ctx.state.complete_step(
            "buildWindows",
            "Windows 打包完成",
            json!({
                "remoteCommit": remote_commit,
                "exportDir": export_dir,
                "retryAfterRustcAccessViolation": retry_output.is_some(),
                "message": command_output_summary(final_output),
                "stdout": final_output.stdout,
                "stderr": final_output.stderr
            }),
        )
    } else {
        ctx.state.fail_step(
            "buildWindows",
            "Windows 打包失败",
            if is_windows_rustc_access_violation(final_output) {
                "Windows rustc 访问冲突自动重试后仍失败，请重启 4090 或更新/修复 Rust toolchain 后重试"
            } else {
                "查看 4090 构建日志，优先处理 npm/tauri/signature 错误"
            },
        )?;
        Err(format!(
            "Windows 打包失败：{}",
            command_output_summary(final_output)
        ))
    }
}

fn command_output_summary(output: &CommandOutput) -> String {
    let combined = format!("{}{}", output.stdout, output.stderr);
    let lines = combined
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if let Some(error) = lines.iter().rev().find(|line| line.starts_with("ERROR:")) {
        return (*error).to_string();
    }
    lines.last().copied().unwrap_or("查看实时日志").to_string()
}

fn collect_windows_artifacts(ctx: &CliContext) -> Result<(), String> {
    ctx.state
        .start_step("collectWindowsArtifacts", "开始收集 Windows 产物")?;
    let target_dir = ctx.release_dir.join("windows");
    fs::create_dir_all(&target_dir).map_err(|error| error.to_string())?;

    let export_dir = windows_export_dir(&ctx.app.config, &ctx.version);
    let remote_zip = join_windows_path(&export_dir, "windows-artifacts.zip");
    let remote_arg = format!(
        "{}:{}",
        ctx.app.config.windows_ssh,
        windows_scp_path(&remote_zip)
    );
    let local_zip = ctx.release_dir.join("windows-artifacts.zip");
    let local_arg = local_zip.to_string_lossy().to_string();
    let output = run_command_output(
        "scp",
        &["-q", remote_arg.as_str(), local_arg.as_str()],
        Some(Path::new(&ctx.app.config.source_repo)),
    )?;
    if output.status != 0 {
        let suggestion = "Windows export zip 拉取失败，请确认 4090 构建脚本已成功导出 windows-artifacts.zip，并检查 scp 权限";
        let summary = command_output_summary(&output);
        ctx.state
            .record_suggestion(suggestion, "error", "collect-windows-artifacts")?;
        ctx.state.fail_step(
            "collectWindowsArtifacts",
            "收集 Windows 产物失败",
            suggestion,
        )?;
        return Err(format!("收集 Windows 产物失败：{remote_zip}\n{summary}"));
    }

    if let Err(error) = unzip_to_dir(&local_zip, &target_dir) {
        let suggestion = "Windows export zip 解压失败，请确认 windows-artifacts.zip 完整且未损坏";
        let _ = fs::remove_file(&local_zip);
        ctx.state
            .record_suggestion(suggestion, "error", "collect-windows-artifacts")?;
        ctx.state.fail_step(
            "collectWindowsArtifacts",
            "解压 Windows 产物失败",
            suggestion,
        )?;
        return Err(format!(
            "解压 Windows 产物失败：{}\n{error}",
            local_zip.display()
        ));
    }
    let _ = fs::remove_file(&local_zip);

    ctx.state.complete_step(
        "collectWindowsArtifacts",
        "Windows 产物已收集",
        json!({
            "source": remote_zip,
            "destination": target_dir.to_string_lossy()
        }),
    )
}

fn build_mac(ctx: &CliContext) -> Result<(), String> {
    ctx.state.start_step("buildMac", "开始 Mac 打包")?;
    let commands = mac_build_command_sequence(&ctx.app.config.source_repo);
    let source_sync = run_command_spec(&commands[0])?;
    if source_sync.status != 0 {
        ctx.state.fail_step(
            "buildMac",
            "拉取源码仓库失败",
            "请处理本机源码仓库的未提交变更、冲突或远端同步问题后重试",
        )?;
        return Err(format!(
            "拉取源码仓库失败\n{}{}",
            source_sync.stdout, source_sync.stderr
        ));
    }
    let latest_commit = run_capture(
        "git",
        &["rev-parse", "HEAD"],
        Some(Path::new(&ctx.app.config.source_repo)),
    )?;
    ctx.state.update_commit(&latest_commit)?;
    let output = run_command_spec(&commands[1])?;
    if output.status != 0 {
        ctx.state.fail_step(
            "buildMac",
            "Mac 打包失败",
            "查看本机 tauri:build:mac 日志并处理依赖、签名或权限错误",
        )?;
        return Err(format!("Mac 打包失败\n{}{}", output.stdout, output.stderr));
    }

    let target_dir = ctx.release_dir.join("mac");
    fs::create_dir_all(&target_dir).map_err(|error| error.to_string())?;
    let mut copied = Vec::new();
    for artifact in ctx
        .app
        .config
        .required_artifacts
        .iter()
        .filter(|artifact| artifact.scope == "mac")
    {
        for (source_template, target_name) in artifact_source_pairs(artifact) {
            let rendered = render_artifact_source(
                &source_template,
                &ctx.version,
                &ctx.app.config.windows_target,
            );
            let source_path = source_artifact_path(&ctx.app.config.source_repo, &rendered);
            if !source_path.exists() {
                ctx.state.record_suggestion(
                    &format!("缺少 Mac 构建输出：{}", source_path.display()),
                    "error",
                    "build-mac",
                )?;
                ctx.state.fail_step(
                    "buildMac",
                    "Mac 构建输出缺失",
                    "检查 tauri:build:mac 输出目录和签名文件",
                )?;
                return Err(format!("缺少 Mac 构建输出：{}", source_path.display()));
            }
            let target_path = target_dir.join(artifact_name(&target_name, &ctx.version));
            fs::copy(&source_path, &target_path).map_err(|error| error.to_string())?;
            copied.push(json!({
                "source": source_path.to_string_lossy(),
                "target": target_path.to_string_lossy()
            }));
        }
    }

    ctx.state.complete_step(
        "buildMac",
        "Mac 打包完成",
        json!({
            "count": copied.len(),
            "files": copied,
            "sourceSync": {
                "after": latest_commit,
                "stdout": source_sync.stdout,
                "stderr": source_sync.stderr
            },
            "stdout": output.stdout,
            "stderr": output.stderr
        }),
    )
}

fn publish_step(ctx: &CliContext, step: &str) -> Result<(), String> {
    let step_key = match step {
        "publish-github" | "publishGithub" => "publishGithub",
        "publish-gitee" | "publishGitee" => "publishGitee",
        _ => return Err(format!("未知发布步骤：{step}")),
    };
    if !confirm_version_matches(&ctx.flags, &ctx.version) {
        ctx.state.fail_step(
            step_key,
            "版本确认不匹配",
            "请输入与目标版本完全一致的 confirmVersion 后再继续",
        )?;
        return Err("版本确认不匹配，已阻止真实发布步骤。".to_string());
    }

    if step_key == "publishGithub" {
        publish_github(ctx)
    } else {
        publish_gitee(ctx)
    }
}

fn publish_github(ctx: &CliContext) -> Result<(), String> {
    ctx.state
        .start_step("publishGithub", "开始发布 GitHub Release")?;
    let assets = publish_asset_paths(&ctx.release_dir, "github")?;
    let args = github_release_create_args(
        &ctx.app.config.github_repo,
        &ctx.version,
        &ctx.state.commit,
        &assets,
    );
    let arg_refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let output = run_command_output(
        "gh",
        &arg_refs,
        Some(Path::new(&ctx.app.config.source_repo)),
    )?;
    if output.status != 0 {
        ctx.state.fail_step(
            "publishGithub",
            "GitHub Release 发布失败",
            "检查 gh 登录状态、tag 是否已存在、仓库权限和待上传产物",
        )?;
        return Err(format!(
            "GitHub Release 发布失败\n{}{}",
            output.stdout, output.stderr
        ));
    }
    ctx.state.complete_step(
        "publishGithub",
        "GitHub Release 已发布",
        json!({
            "host": "github",
            "tag": format!("v{}", ctx.version),
            "repo": ctx.app.config.github_repo,
            "assetCount": assets.len(),
            "assets": assets.iter().map(|path| path.to_string_lossy().to_string()).collect::<Vec<_>>(),
            "stdout": output.stdout,
            "stderr": output.stderr
        }),
    )
}

fn publish_gitee(ctx: &CliContext) -> Result<(), String> {
    ctx.state
        .start_step("publishGitee", "开始发布 Gitee Release")?;
    let token = match gitee_access_token() {
        Ok(token) => token,
        Err(error) => {
            ctx.state.fail_step(
                "publishGitee",
                "Gitee 凭据不可用",
                "检查 git credential 中的 Gitee access token",
            )?;
            return Err(error);
        }
    };
    let assets = publish_asset_paths(&ctx.release_dir, "gitee")?;
    let release = match create_gitee_release(ctx, &token) {
        Ok(release) => release,
        Err(error) => {
            ctx.state.fail_step(
                "publishGitee",
                "Gitee Release 创建失败",
                "检查 Gitee token 权限、tag 是否已存在和仓库 API 可用性",
            )?;
            return Err(error);
        }
    };
    let release_id = release["id"]
        .as_i64()
        .ok_or_else(|| "Gitee Release 响应缺少 id".to_string())?;
    let mut uploaded = Vec::new();
    for asset in &assets {
        match upload_gitee_release_asset(ctx, &token, release_id, asset) {
            Ok(result) => uploaded.push(result),
            Err(error) => {
                ctx.state.fail_step(
                    "publishGitee",
                    "Gitee Release 附件上传失败",
                    "检查 Gitee 附件大小限制、网络状态和 token 权限",
                )?;
                return Err(error);
            }
        }
    }
    let latest = match update_gitee_latest_json(ctx, &token) {
        Ok(result) => result,
        Err(error) => {
            ctx.state.fail_step(
                "publishGitee",
                "Gitee latest.json 更新失败",
                "检查 Gitee contents API 权限和 master 分支状态",
            )?;
            return Err(error);
        }
    };
    ctx.state.complete_step(
        "publishGitee",
        "Gitee Release 已发布",
        json!({
            "host": "gitee",
            "tag": format!("v{}", ctx.version),
            "repo": format!("{}/{}", ctx.app.config.gitee_owner, ctx.app.config.gitee_repo),
            "releaseId": release_id,
            "assetCount": uploaded.len(),
            "assets": uploaded,
            "latest": latest
        }),
    )
}

fn publish_asset_paths(release_dir: &Path, host: &str) -> Result<Vec<PathBuf>, String> {
    let manifest = release_dir.join(host).join("latest.json");
    ensure(
        manifest.exists(),
        &format!("缺少 {host} manifest：{}", manifest.display()),
    )?;
    let mut assets = vec![manifest];
    for scope in ["windows", "mac"] {
        let mut files = list_files(&release_dir.join(scope))?;
        files.sort();
        assets.extend(files);
    }
    ensure(!assets.is_empty(), "没有可发布产物")?;
    for asset in &assets {
        ensure(
            asset.exists() && asset.is_file(),
            &format!("缺少待发布文件：{}", asset.display()),
        )?;
    }
    Ok(assets)
}

fn github_release_create_args(
    repo: &str,
    version: &str,
    commit: &str,
    assets: &[PathBuf],
) -> Vec<String> {
    let mut args = vec![
        "release".to_string(),
        "create".to_string(),
        format!("v{version}"),
    ];
    args.extend(
        assets
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>(),
    );
    args.extend([
        "--repo".to_string(),
        repo.to_string(),
        "--title".to_string(),
        format!("OMD {version}"),
        "--notes".to_string(),
        format!("OMD {version}"),
    ]);
    if !commit.is_empty() {
        args.extend(["--target".to_string(), commit.to_string()]);
    }
    args
}

fn gitee_release_payload(version: &str, commit: &str) -> Value {
    json!({
        "tag_name": format!("v{version}"),
        "name": format!("OMD {version}"),
        "body": format!("OMD {version}"),
        "target_commitish": commit,
        "prerelease": false
    })
}

fn create_gitee_release(ctx: &CliContext, token: &str) -> Result<Value, String> {
    let mut payload = gitee_release_payload(&ctx.version, &ctx.state.commit);
    payload["access_token"] = json!(token);
    let body = serde_json::to_string(&payload).map_err(|error| error.to_string())?;
    let url = format!(
        "https://gitee.com/api/v5/repos/{}/{}/releases",
        ctx.app.config.gitee_owner, ctx.app.config.gitee_repo
    );
    let output = run_command_input_output(
        "curl",
        &[
            "-sS",
            "--fail-with-body",
            "-X",
            "POST",
            &url,
            "-H",
            "Content-Type: application/json",
            "-d",
            "@-",
        ],
        &body,
        None,
    )?;
    if output.status != 0 {
        return Err(format!(
            "Gitee Release 创建失败\n{}{}",
            output.stdout, output.stderr
        ));
    }
    serde_json::from_str(&output.stdout).map_err(|error| error.to_string())
}

fn upload_gitee_release_asset(
    ctx: &CliContext,
    token: &str,
    release_id: i64,
    asset: &Path,
) -> Result<Value, String> {
    let url = format!(
        "https://gitee.com/api/v5/repos/{}/{}/releases/{}/attach_files",
        ctx.app.config.gitee_owner, ctx.app.config.gitee_repo, release_id
    );
    let config = format!(
        "url = \"{}\"\nrequest = \"POST\"\nform = \"access_token={}\"\nform = \"file=@{}\"\n",
        curl_config_escape(&url),
        curl_config_escape(token),
        curl_config_escape(&asset.to_string_lossy())
    );
    let output = run_command_input_output(
        "curl",
        &["-sS", "--fail-with-body", "-K", "-"],
        &config,
        None,
    )?;
    if output.status != 0 {
        return Err(format!(
            "Gitee 附件上传失败：{}\n{}{}",
            asset.display(),
            output.stdout,
            output.stderr
        ));
    }
    let result: Value = serde_json::from_str(&output.stdout).map_err(|error| error.to_string())?;
    Ok(json!({
        "name": result["name"].as_str().unwrap_or_else(|| asset.file_name().and_then(|name| name.to_str()).unwrap_or("")),
        "id": result["id"].clone(),
        "size": result["size"].clone()
    }))
}

fn update_gitee_latest_json(ctx: &CliContext, token: &str) -> Result<Value, String> {
    let file = ctx.release_dir.join("gitee/latest.json");
    ensure(
        file.exists(),
        &format!("缺少 Gitee latest.json：{}", file.display()),
    )?;
    let content = base64_file(&file)?;
    let sha = gitee_latest_sha(ctx, token)?;
    let mut payload = json!({
        "access_token": token,
        "content": content,
        "message": format!("Update OMD {} latest.json", ctx.version),
        "branch": "master"
    });
    let method = if let Some(existing_sha) = sha {
        payload["sha"] = json!(existing_sha);
        "PUT"
    } else {
        "POST"
    };
    let body = serde_json::to_string(&payload).map_err(|error| error.to_string())?;
    let url = format!(
        "https://gitee.com/api/v5/repos/{}/{}/contents/latest.json",
        ctx.app.config.gitee_owner, ctx.app.config.gitee_repo
    );
    let output = run_command_input_output(
        "curl",
        &[
            "-sS",
            "--fail-with-body",
            "-X",
            method,
            &url,
            "-H",
            "Content-Type: application/json",
            "-d",
            "@-",
        ],
        &body,
        None,
    )?;
    if output.status != 0 {
        return Err(format!(
            "Gitee latest.json 更新失败\n{}{}",
            output.stdout, output.stderr
        ));
    }
    let result: Value = serde_json::from_str(&output.stdout).map_err(|error| error.to_string())?;
    Ok(json!({
        "method": method,
        "path": "latest.json",
        "commit": result["commit"]["sha"].as_str().unwrap_or("")
    }))
}

fn gitee_latest_sha(ctx: &CliContext, token: &str) -> Result<Option<String>, String> {
    let url = format!(
        "https://gitee.com/api/v5/repos/{}/{}/contents/latest.json",
        ctx.app.config.gitee_owner, ctx.app.config.gitee_repo
    );
    let config = format!(
        "url = \"{}\"\ndata-urlencode = \"access_token={}\"\ndata-urlencode = \"ref=master\"\n",
        curl_config_escape(&url),
        curl_config_escape(token)
    );
    let output = run_command_input_output(
        "curl",
        &[
            "-sS",
            "-w",
            "\nOMD_HTTP_STATUS:%{http_code}",
            "-G",
            "-K",
            "-",
        ],
        &config,
        None,
    )?;
    if output.status != 0 {
        return Err(format!(
            "查询 Gitee latest.json 失败\n{}{}",
            output.stdout, output.stderr
        ));
    }
    let (body, status) = split_curl_status(&output.stdout)?;
    parse_gitee_latest_sha(status, body.trim())
}

fn split_curl_status(stdout: &str) -> Result<(&str, u16), String> {
    let Some((body, raw_status)) = stdout.rsplit_once("\nOMD_HTTP_STATUS:") else {
        return Err("curl 响应缺少 HTTP 状态码".to_string());
    };
    let status = raw_status
        .trim()
        .parse::<u16>()
        .map_err(|error| error.to_string())?;
    Ok((body, status))
}

fn parse_gitee_latest_sha(status: u16, body: &str) -> Result<Option<String>, String> {
    if status == 404 {
        return Ok(None);
    }
    if !(200..300).contains(&status) {
        return Err(format!("查询 Gitee latest.json 返回 HTTP {status}: {body}"));
    }
    let result: Value = serde_json::from_str(body).map_err(|error| error.to_string())?;
    Ok(result["sha"].as_str().map(ToString::to_string))
}

fn base64_file(file: &Path) -> Result<String, String> {
    run_capture("base64", &["-i", &file.to_string_lossy()], None)
        .map(|value| value.replace(['\r', '\n'], ""))
}

fn curl_config_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn dry_run_commands(ctx: &CliContext) -> Result<Vec<String>, String> {
    let release_dir = ctx.release_dir.to_string_lossy();
    let remote_script = windows_build_script(&ctx.app.config);
    let remote_export_zip = join_windows_path(
        &windows_export_dir(&ctx.app.config, &ctx.version),
        "windows-artifacts.zip",
    );
    let build_invocation = windows_build_script_invocation(&ctx.app.config, &ctx.version, false);
    Ok(vec![
        format!(
            "scp scripts/windows/omd-release-build.ps1 {}:{}",
            ctx.app.config.windows_ssh,
            windows_scp_path(&remote_script)
        ),
        format!(
            "ssh {} \"powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command \"{}\"\"",
            ctx.app.config.windows_ssh, build_invocation
        ),
        format!(
            "scp {}:{} {release_dir}/windows-artifacts.zip",
            ctx.app.config.windows_ssh,
            windows_scp_path(&remote_export_zip)
        ),
        format!(
            "python3 -m zipfile -e {release_dir}/windows-artifacts.zip {release_dir}/windows/"
        ),
        format!("git -C {} pull --ff-only", ctx.app.config.source_repo),
        format!(
            "npm --prefix {} run tauri:build:mac",
            ctx.app.config.source_repo
        ),
        format!(
            "cargo run -- manifest {} --release-dir {release_dir}",
            ctx.version
        ),
        format!("gh release create v{} {release_dir}/github/latest.json {release_dir}/windows/* {release_dir}/mac/* -R {} --title \"OMD {}\"", ctx.version, ctx.app.config.github_repo, ctx.version),
        format!(
            "cargo run -- verify {} --release-dir {release_dir}",
            ctx.version
        ),
    ])
}

fn artifact_source_pairs(artifact: &RequiredArtifact) -> Vec<(String, String)> {
    vec![
        (
            artifact
                .source_file
                .clone()
                .unwrap_or_else(|| artifact.file.clone()),
            artifact.file.clone(),
        ),
        (
            artifact
                .source_signature
                .clone()
                .unwrap_or_else(|| artifact.signature.clone()),
            artifact.signature.clone(),
        ),
    ]
}

fn join_windows_path(root: &str, relative: &str) -> String {
    let relative = relative.replace('/', "\\");
    format!("{}\\{}", root.trim_end_matches('\\'), relative)
}

fn windows_scp_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    if normalized.len() >= 2 && normalized.as_bytes()[1] == b':' {
        format!("/{normalized}")
    } else {
        normalized
    }
}

fn unzip_to_dir(zip_file: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|error| error.to_string())?;
    let zip_arg = zip_file.to_string_lossy().to_string();
    let dest_arg = destination.to_string_lossy().to_string();
    let output = run_command_output(
        "python3",
        &[
            "-c",
            "import os, sys, zipfile\nzip_path, dest = sys.argv[1], os.path.abspath(sys.argv[2])\nwith zipfile.ZipFile(zip_path) as z:\n    for info in z.infolist():\n        target = os.path.abspath(os.path.join(dest, info.filename))\n        if target != dest and not target.startswith(dest + os.sep):\n            raise SystemExit('unsafe zip entry: ' + info.filename)\n    z.extractall(dest)",
            zip_arg.as_str(),
            dest_arg.as_str(),
        ],
        None,
    )?;
    if output.status != 0 {
        return Err(command_output_summary(&output));
    }
    Ok(())
}

fn windows_build_script(config: &ReleaseConfig) -> String {
    config
        .windows_build_script
        .clone()
        .unwrap_or_else(|| r"C:\Users\4090\Desktop\omd-release-build.ps1".to_string())
}

fn windows_build_dir(config: &ReleaseConfig) -> String {
    config
        .windows_build_dir
        .clone()
        .unwrap_or_else(|| r"C:\Users\4090\Desktop\dix-extension-ui-release-build".to_string())
}

fn windows_export_dir(config: &ReleaseConfig, version: &str) -> String {
    config
        .windows_export_dir_pattern
        .as_deref()
        .unwrap_or(r"C:\Users\4090\AppData\Local\Temp\omd-release-export-{version}")
        .replace("{version}", version)
}

fn windows_branch(config: &ReleaseConfig) -> String {
    config
        .windows_branch
        .clone()
        .unwrap_or_else(|| "codex/tauri-rust-migration".to_string())
}

fn local_windows_build_script(root: &Path) -> PathBuf {
    root.join("scripts/windows/omd-release-build.ps1")
}

fn windows_build_script_invocation(config: &ReleaseConfig, version: &str, clean: bool) -> String {
    let mut script = format!(
        "& '{}' -Version '{}' -Repo '{}' -BuildDir '{}' -Branch '{}' -ExportDir '{}'",
        powershell_escape(&windows_build_script(config)),
        powershell_escape(version),
        powershell_escape(&config.windows_repo),
        powershell_escape(&windows_build_dir(config)),
        powershell_escape(&windows_branch(config)),
        powershell_escape(&windows_export_dir(config, version)),
    );
    if clean {
        script.push_str(" -Clean");
    }
    script
}

fn powershell_escape(value: &str) -> String {
    value.replace('`', "``").replace('\'', "''")
}

fn source_artifact_path(source_repo: &str, rendered: &str) -> PathBuf {
    let path = expand_home(rendered);
    if path.is_absolute() {
        path
    } else {
        Path::new(source_repo).join(path)
    }
}

fn check_artifacts(ctx: &CliContext) -> Result<(), String> {
    fs::create_dir_all(&ctx.release_dir).map_err(|error| error.to_string())?;
    ctx.state.start_step("artifactCheck", "检查发布产物")?;
    let artifacts = inspect_artifacts(ctx)?;
    ctx.state.record_artifacts(artifacts.clone())?;
    let missing = artifacts
        .iter()
        .filter(|artifact| {
            artifact["required"].as_bool().unwrap_or(false)
                && !artifact["exists"].as_bool().unwrap_or(false)
        })
        .count();
    if missing > 0 {
        let message = format!("缺少 {missing} 个必需产物或签名");
        for artifact in artifacts.iter().filter(|artifact| {
            artifact["required"].as_bool().unwrap_or(false)
                && !artifact["exists"].as_bool().unwrap_or(false)
        }) {
            ctx.state.record_suggestion(
                &missing_artifact_message(artifact),
                "error",
                "check-artifacts",
            )?;
        }
        ctx.state
            .fail_step("artifactCheck", &message, "请补齐缺失产物后重新检查产物")?;
        return Err(message);
    }
    ctx.state.complete_step(
        "artifactCheck",
        "产物完整",
        json!({ "count": artifacts.len() }),
    )
}

fn missing_artifact_message(artifact: &Value) -> String {
    format!(
        "缺少 {}/{}，请回到对应构建机补齐产物或签名。",
        artifact["scope"].as_str().unwrap_or("unknown"),
        artifact["name"].as_str().unwrap_or("unknown")
    )
}

fn report(ctx: &CliContext) -> Result<(), String> {
    ctx.state.start_step("report", "生成发布报告")?;
    let state = ctx.state.load_state();
    let body = release_report_markdown(&state);
    let file = ctx.release_dir.join("release-report.md");
    fs::write(&file, body).map_err(|error| error.to_string())?;
    ctx.state.complete_step(
        "report",
        "发布报告已生成",
        json!({ "file": file.to_string_lossy() }),
    )
}

fn release_report_markdown(state: &Value) -> String {
    let version = state["version"].as_str().unwrap_or("unknown");
    let commit = state["commit"].as_str().unwrap_or("");
    let mut body = format!("# OMD {version} Release Report\n\nCommit: `{commit}`\n\n## Steps\n\n");
    if let Some(steps) = state["steps"].as_object() {
        for (key, step) in steps {
            body.push_str(&format!(
                "- `{}`: {} — {}\n",
                key,
                step["status"].as_str().unwrap_or("not_started"),
                step["message"].as_str().unwrap_or("")
            ));
        }
    }
    body
}

fn release_history_file(data_dir: &Path) -> PathBuf {
    data_dir.join("release-history.json")
}

fn load_release_history(data_dir: &Path) -> Value {
    read_json_if_exists(&release_history_file(data_dir)).unwrap_or_else(|| json!({ "runs": [] }))
}

fn record_release_history(app: &AppContext, state: &Value) -> Result<(), String> {
    fs::create_dir_all(&app.data_dir).map_err(|error| error.to_string())?;
    let mut history = load_release_history(&app.data_dir);
    ensure_array(&mut history, "runs");
    upsert_by_key(
        &mut history["runs"],
        json!({
            "key": state["runId"].as_str().unwrap_or(""),
            "runId": state["runId"].as_str().unwrap_or(""),
            "version": state["version"].as_str().unwrap_or(""),
            "releaseDir": state["releaseDir"].as_str().unwrap_or(""),
            "overallStatus": state["overallStatus"].as_str().unwrap_or("not_started"),
            "updatedAt": state["updatedAt"].as_str().unwrap_or("")
        }),
    );
    let body = serde_json::to_string_pretty(&history).map_err(|error| error.to_string())?;
    fs::write(release_history_file(&app.data_dir), format!("{body}\n"))
        .map_err(|error| error.to_string())
}

fn checksums(ctx: &CliContext) -> Result<(), String> {
    check_artifacts(ctx)?;

    ctx.state.start_step("checksums", "生成 checksums.sha256")?;
    let files: Vec<PathBuf> = list_files(&ctx.release_dir)?
        .into_iter()
        .filter(|file| file.file_name() != Some(OsStr::new("checksums.sha256")))
        .collect();
    let mut body = String::new();
    for file in &files {
        let rel = file.strip_prefix(&ctx.release_dir).unwrap_or(file);
        body.push_str(&format!("{}  {}\n", sha256(file)?, rel.to_string_lossy()));
    }
    fs::write(ctx.release_dir.join("checksums.sha256"), body).map_err(|error| error.to_string())?;
    ctx.state.complete_step(
        "checksums",
        "checksums.sha256 已生成",
        json!({ "count": files.len() }),
    )?;
    progress(&format!(
        "已写入 {}",
        ctx.release_dir.join("checksums.sha256").display()
    ));
    Ok(())
}

fn manifests(ctx: &CliContext) -> Result<(), String> {
    for host in ["github", "gitee"] {
        let step_key = if host == "github" {
            "manifestGithub"
        } else {
            "manifestGitee"
        };
        ctx.state
            .start_step(step_key, &format!("生成 {host} manifest"))?;
        let dir = ctx.release_dir.join(host);
        fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        let manifest = manifest_for(ctx, host)?;
        let file = dir.join("latest.json");
        let body = serde_json::to_string_pretty(&manifest).map_err(|error| error.to_string())?;
        fs::write(&file, format!("{body}\n")).map_err(|error| error.to_string())?;
        validate_manifest(ctx, &file)?;
        let summary = manifest_summary_for_state(&manifest);
        ctx.state.record_manifest(host, summary.clone())?;
        ctx.state
            .complete_step(step_key, &format!("{host} manifest 已生成"), summary)?;
        progress(&format!("已写入 {}", file.display()));
    }
    Ok(())
}

fn inspect_artifacts(ctx: &CliContext) -> Result<Vec<Value>, String> {
    let mut artifacts = Vec::new();
    for target in &ctx.app.config.required_artifacts {
        let file_name = artifact_name(&target.file, &ctx.version);
        let sig_name = artifact_name(&target.signature, &ctx.version);
        let file_path = ctx.release_dir.join(&target.scope).join(&file_name);
        let sig_path = ctx.release_dir.join(&target.scope).join(&sig_name);
        let file_info = fs::metadata(&file_path).ok();
        let sig_info = fs::metadata(&sig_path).ok();
        let file_exists = file_info.is_some();
        let sig_exists = sig_info.is_some();
        artifacts.push(json!({
            "key": target.key,
            "scope": target.scope,
            "name": file_name,
            "required": true,
            "exists": file_exists,
            "size": file_info.as_ref().map(|info| info.len()).unwrap_or(0),
            "modifiedAt": file_info.as_ref().map(|info| iso_from_system_time(info.modified().unwrap_or(SystemTime::UNIX_EPOCH))).unwrap_or_default(),
            "checksum": if file_exists { sha256(&file_path)? } else { String::new() },
            "signatureFor": "",
            "hasSignature": sig_exists
        }));
        artifacts.push(json!({
            "key": format!("{}:signature", target.key),
            "scope": target.scope,
            "name": sig_name,
            "required": true,
            "exists": sig_exists,
            "size": sig_info.as_ref().map(|info| info.len()).unwrap_or(0),
            "modifiedAt": sig_info.as_ref().map(|info| iso_from_system_time(info.modified().unwrap_or(SystemTime::UNIX_EPOCH))).unwrap_or_default(),
            "checksum": if sig_exists { sha256(&sig_path)? } else { String::new() },
            "signatureFor": file_name,
            "hasSignature": false
        }));
    }
    Ok(artifacts)
}

fn manifest_for(ctx: &CliContext, host: &str) -> Result<Value, String> {
    let base = if host == "github" {
        format!(
            "https://github.com/{}/releases/download/v{}",
            ctx.app.config.github_repo, ctx.version
        )
    } else {
        format!(
            "https://gitee.com/{}/{}/releases/download/v{}",
            ctx.app.config.gitee_owner, ctx.app.config.gitee_repo, ctx.version
        )
    };
    let mut platforms = Map::new();
    for target in &ctx.app.config.required_artifacts {
        let sig_path = ctx
            .release_dir
            .join(&target.scope)
            .join(artifact_name(&target.signature, &ctx.version));
        ensure(
            sig_path.exists(),
            &format!("缺少签名文件：{}", sig_path.display()),
        )?;
        let signature = fs::read_to_string(&sig_path)
            .map_err(|error| error.to_string())?
            .replace(['\r', '\n'], "");
        platforms.insert(
            target.key.clone(),
            json!({
                "signature": signature,
                "url": format!("{}/{}", base, artifact_name(&target.file, &ctx.version))
            }),
        );
    }
    Ok(json!({
        "version": ctx.version,
        "notes": format!("OMD {}", ctx.version),
        "pub_date": iso_now(),
        "platforms": platforms
    }))
}

fn validate_manifest(ctx: &CliContext, file: &Path) -> Result<(), String> {
    let bytes = fs::read(file).map_err(|error| error.to_string())?;
    ensure(
        !(bytes.len() >= 3 && bytes[0] == 0xef && bytes[1] == 0xbb && bytes[2] == 0xbf),
        &format!("{} 含有 BOM", file.display()),
    )?;
    let manifest: Value = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    ensure(
        manifest["platforms"].get("windows-x86_64").is_none(),
        "不能发布通用 windows-x86_64 target",
    )?;
    for target in &ctx.app.config.required_artifacts {
        ensure(
            manifest["platforms"][&target.key]["url"].is_string(),
            &format!("{} 缺少 url", target.key),
        )?;
        ensure(
            manifest["platforms"][&target.key]["signature"].is_string(),
            &format!("{} 缺少 signature", target.key),
        )?;
    }
    Ok(())
}

fn manifest_summary_for_state(manifest: &Value) -> Value {
    let platforms = manifest["platforms"]
        .as_object()
        .map(|items| {
            items
                .iter()
                .map(|(key, value)| {
                    json!({
                        "key": key,
                        "url": value["url"].as_str().unwrap_or(""),
                        "hasSignature": value["signature"].as_str().map(|item| !item.is_empty()).unwrap_or(false)
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({
        "exists": true,
        "version": manifest["version"].as_str().unwrap_or(""),
        "pubDate": manifest["pub_date"].as_str().unwrap_or(""),
        "platforms": platforms
    })
}

fn verify(ctx: &CliContext) -> Result<(), String> {
    ctx.state.start_step("verify", "验证 updater 端点")?;
    let mut results = Vec::new();
    for endpoint in [&ctx.app.config.gitee_latest, &ctx.app.config.github_latest] {
        match verify_endpoint(ctx, endpoint) {
            Ok(result) => {
                progress(&format!("已验证 {endpoint}"));
                results.push(result);
            }
            Err(error) => {
                results.push(json!({ "endpoint": endpoint, "status": "failed", "error": error, "platforms": [] }));
                ctx.state.record_suggestion(
                    &format!("请检查 updater endpoint：{endpoint}"),
                    "error",
                    "verify",
                )?;
            }
        }
    }
    ctx.state
        .record_manifest("onlineVerify", json!({ "results": results }))?;
    let state = ctx.state.load_state();
    let failed = state["manifests"]["onlineVerify"]["results"]
        .as_array()
        .map(|items| items.iter().any(|item| item["status"] == "failed"))
        .unwrap_or(false);
    if failed {
        ctx.state.fail_step(
            "verify",
            "线上验证失败",
            "请检查失败的 updater endpoint 或下载 URL",
        )?;
        return Err("线上验证失败".to_string());
    }
    let count = state["manifests"]["onlineVerify"]["results"]
        .as_array()
        .map(|items| items.len())
        .unwrap_or(0);
    ctx.state
        .complete_step("verify", "线上验证通过", json!({ "endpoints": count }))?;
    Ok(())
}

fn verify_endpoint(ctx: &CliContext, endpoint: &str) -> Result<Value, String> {
    let manifest = fetch_json(ctx, endpoint)?;
    let version = manifest["version"].as_str().unwrap_or("");
    ensure(
        version == ctx.version,
        &format!("{endpoint} 返回版本 {version}"),
    )?;
    let mut platforms = Vec::new();
    if let Some(items) = manifest["platforms"].as_object() {
        for (key, value) in items {
            ensure(
                value["signature"]
                    .as_str()
                    .map(|item| !item.is_empty())
                    .unwrap_or(false),
                &format!("{key} 缺少 signature"),
            )?;
            let url = value["url"].as_str().unwrap_or("");
            let status = http_status(ctx, url)?;
            ensure(status == 200, &format!("{url} 无法访问"))?;
            platforms.push(
                json!({ "key": key, "url": url, "httpStatus": status, "hasSignature": true }),
            );
        }
    }
    Ok(
        json!({ "endpoint": endpoint, "status": "success", "version": version, "platforms": platforms }),
    )
}

fn serve(app: AppContext) -> Result<(), String> {
    let port = env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(4177);
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|error| error.to_string())?;
    println!("OMD 发布控制台已启动：http://127.0.0.1:{port}");
    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                if let Err(error) = handle_connection(&app, &mut stream) {
                    let _ = send_json(&mut stream, 500, &json!({ "ok": false, "error": error }));
                }
            }
            Err(error) => eprintln!("连接失败：{error}"),
        }
    }
    Ok(())
}

fn handle_connection(app: &AppContext, stream: &mut TcpStream) -> Result<(), String> {
    let request = read_http_request(stream)?;
    let (method, target) = parse_request_line(&request)?;
    let (path, query) = split_query(&target);
    let body = request_body(&request);

    if path == "/api/summary" {
        let release_dir = query_param(&query, "releaseDir")
            .filter(|value| !value.is_empty())
            .map(|value| expand_home(&value))
            .or_else(|| find_latest_release_dir(&app.desktop_dir));
        return send_json(
            stream,
            200,
            &summarize_release(app, release_dir.as_deref())?,
        );
    }

    if path == "/api/commands" {
        let version = query_param(&query, "version")
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "0.0.7".to_string());
        return send_json(stream, 200, &release_commands(&version));
    }

    if path == "/api/config" {
        return send_json(
            stream,
            200,
            &json!({ "ok": true, "config": config_summary(&app.config) }),
        );
    }

    if path == "/api/history" {
        return send_json(stream, 200, &load_release_history(&app.data_dir));
    }

    if path == "/api/action-log" {
        let body_json = json!({
            "version": query_param(&query, "version").unwrap_or_default(),
            "releaseDir": query_param(&query, "releaseDir").unwrap_or_default(),
        });
        let action = query_param(&query, "action").unwrap_or_default();
        return send_json(stream, 200, &action_log_response(app, &action, &body_json));
    }

    if path.starts_with("/api/actions/") {
        if method != "POST" {
            return send_json(
                stream,
                405,
                &json!({ "ok": false, "error": "Action API 只支持 POST" }),
            );
        }
        let action = path.trim_start_matches("/api/actions/");
        let body_json: Value = if body.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(body.trim()).map_err(|error| error.to_string())?
        };
        return send_json(stream, 200, &run_action(app, action, &body_json)?);
    }

    serve_static(app, stream, &path)
}

fn action_log_response(app: &AppContext, action: &str, body: &Value) -> Value {
    let Ok((_, release_dir)) = validate_action_input(app, body) else {
        return json!({ "ok": false, "log": "", "message": "releaseDir 无效" });
    };
    let file = action_log_file(&release_dir, action);
    let log = fs::read_to_string(&file).unwrap_or_default();
    json!({
        "ok": true,
        "action": action,
        "log": log,
        "logFile": file.to_string_lossy()
    })
}

fn run_action(app: &AppContext, action: &str, body: &Value) -> Result<Value, String> {
    let (version, release_dir) = match validate_action_input(app, body) {
        Ok(value) => value,
        Err(message) => return Ok(action_error(action, "invalid_input", &message)),
    };
    let state = read_json_if_exists(&release_dir.join("state.json")).unwrap_or_else(|| json!({}));

    if let Some(message) = missing_action_prerequisite(action, &state) {
        return Ok(action_error(action, "blocked", &message));
    }

    if action == "publish-github" || action == "publish-gitee" {
        let confirm = body["confirmVersion"].as_str().unwrap_or("").trim();
        if confirm != version {
            return Ok(action_error(
                action,
                "invalid_confirm",
                "confirmVersion 必须与目标 version 完全一致。",
            ));
        }
    }

    let command_args = match action_command_args(action, &version, &release_dir) {
        Ok(args) => args,
        Err(error) => return Ok(error),
    };
    let binary = env::current_exe().map_err(|error| error.to_string())?;
    let log_file = action_log_file(&release_dir, action);
    initialize_action_log(&log_file, action, &command_args)?;
    let output = run_binary(&binary, &command_args, &app.root, &log_file)?;
    let updated_state = read_json_if_exists(&release_dir.join("state.json"));
    if let Some(state_value) = updated_state.as_ref() {
        let _ = record_release_history(app, state_value);
    }
    let manual_required =
        output.stderr.contains("真实上传仍需人工执行") || output.stdout.contains("[需人工确认]");
    let ok = output.status == 0;
    let status = if ok {
        "success"
    } else if manual_required {
        "manual_required"
    } else {
        "failed"
    };
    Ok(action_success(
        action,
        status,
        if ok {
            format!("{action} 完成")
        } else if manual_required {
            format!("{action} 需要人工执行")
        } else {
            format!("{action} 失败")
        },
        updated_state,
        output,
    ))
}

fn missing_action_prerequisite(action: &str, state: &Value) -> Option<String> {
    let missing = match action {
        "preflight" | "dry-run-release" => return None,
        "build-windows" | "build-mac" => first_missing_step(state, &["preflight"]),
        "collect-windows-artifacts" => {
            first_missing_step(state, &["preflight", "buildWindows", "buildMac"])
        }
        "check-artifacts" => first_missing_step(
            state,
            &[
                "preflight",
                "buildWindows",
                "buildMac",
                "collectWindowsArtifacts",
            ],
        ),
        "manifest" => first_missing_step(
            state,
            &[
                "preflight",
                "buildWindows",
                "buildMac",
                "collectWindowsArtifacts",
                "artifactCheck",
            ],
        ),
        "publish-github" => first_missing_step(state, &["manifestGithub"]),
        "publish-gitee" => first_missing_step(state, &["manifestGitee"]),
        "verify" => first_missing_step(state, &["publishGithub", "publishGitee"]),
        "report" => first_missing_step(state, &["verify"]),
        _ => return None,
    }?;
    Some(format!(
        "请先完成{}，再执行当前动作。",
        step_display_name(missing)
    ))
}

fn first_missing_step<'a>(state: &Value, steps: &'a [&'a str]) -> Option<&'a str> {
    steps
        .iter()
        .copied()
        .find(|step| state["steps"][*step]["status"].as_str() != Some("success"))
}

fn step_display_name(step: &str) -> &'static str {
    match step {
        "preflight" => "预检",
        "buildWindows" => "Windows 打包",
        "buildMac" => "Mac 打包",
        "collectWindowsArtifacts" => "Windows 产物收集",
        "artifactCheck" => "产物检查",
        "checksums" => "checksums 生成",
        "manifestGithub" => "GitHub Manifest",
        "manifestGitee" => "Gitee Manifest",
        "publishGithub" => "GitHub 发布",
        "publishGitee" => "Gitee 发布",
        "verify" => "线上验证",
        _ => "前置步骤",
    }
}

fn action_success(
    action: &str,
    status: &str,
    message: String,
    state: Option<Value>,
    output: CommandOutput,
) -> Value {
    json!({
        "ok": status == "success" || status == "manual_required",
        "action": action,
        "status": status,
        "message": message,
        "state": state,
        "logs": { "stdout": output.stdout, "stderr": output.stderr },
        "suggestions": state.as_ref().and_then(|item| item["suggestions"].as_array()).cloned().unwrap_or_default()
    })
}

fn action_log_file(release_dir: &Path, action: &str) -> PathBuf {
    release_dir
        .join("logs")
        .join(format!("{}.log", safe_log_name(action)))
}

fn safe_log_name(value: &str) -> String {
    let name = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    if name.is_empty() {
        "action".to_string()
    } else {
        name
    }
}

fn initialize_action_log(log_file: &Path, action: &str, args: &[String]) -> Result<(), String> {
    if let Some(parent) = log_file.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let body = format!(
        "== OMD action: {action} ==\nstartedAt: {}\ncommand: {}\n\n",
        iso_now(),
        args.join(" ")
    );
    fs::write(log_file, body).map_err(|error| error.to_string())
}

fn action_command_args(
    action: &str,
    version: &str,
    release_dir: &Path,
) -> Result<Vec<String>, Value> {
    let release_dir = release_dir.to_string_lossy().to_string();
    match action {
        "preflight" => Ok(vec![
            "preflight".to_string(),
            version.to_string(),
            "--release-dir".to_string(),
            release_dir,
        ]),
        "build-windows" => Ok(vec![
            "build-windows".to_string(),
            version.to_string(),
            "--release-dir".to_string(),
            release_dir,
        ]),
        "collect-windows-artifacts" => Ok(vec![
            "collect-windows-artifacts".to_string(),
            version.to_string(),
            "--release-dir".to_string(),
            release_dir,
        ]),
        "build-mac" => Ok(vec![
            "build-mac".to_string(),
            version.to_string(),
            "--release-dir".to_string(),
            release_dir,
        ]),
        "check-artifacts" => Ok(vec![
            "check-artifacts".to_string(),
            version.to_string(),
            "--release-dir".to_string(),
            release_dir,
        ]),
        "manifest" => Ok(vec![
            "manifest".to_string(),
            version.to_string(),
            "--release-dir".to_string(),
            release_dir,
        ]),
        "verify" => Ok(vec![
            "verify".to_string(),
            version.to_string(),
            "--release-dir".to_string(),
            release_dir,
        ]),
        "dry-run-release" => Ok(vec![
            "release".to_string(),
            version.to_string(),
            "--release-dir".to_string(),
            release_dir,
            "--dry-run".to_string(),
        ]),
        "report" => Ok(vec![
            "report".to_string(),
            version.to_string(),
            "--release-dir".to_string(),
            release_dir,
        ]),
        "publish-github" => Ok(vec![
            "release".to_string(),
            version.to_string(),
            "--release-dir".to_string(),
            release_dir,
            "--step".to_string(),
            "publish-github".to_string(),
            "--confirm-version".to_string(),
            version.to_string(),
        ]),
        "publish-gitee" => Ok(vec![
            "release".to_string(),
            version.to_string(),
            "--release-dir".to_string(),
            release_dir,
            "--step".to_string(),
            "publish-gitee".to_string(),
            "--confirm-version".to_string(),
            version.to_string(),
        ]),
        _ => Err(action_error(
            action,
            "unknown_action",
            &format!("未知 action：{action}"),
        )),
    }
}

fn validate_action_input(app: &AppContext, body: &Value) -> Result<(String, PathBuf), String> {
    let version = body["version"].as_str().unwrap_or("").trim().to_string();
    let release_dir_value = body["releaseDir"].as_str().unwrap_or("").trim().to_string();

    if !is_version(&version) {
        return Err("请提供 SemVer 版本号，例如 0.0.7。".to_string());
    }
    if release_dir_value.is_empty() {
        return Err("请提供 releaseDir。".to_string());
    }
    let release_dir = absolute_path(&expand_home(&release_dir_value));
    if !is_allowed_release_dir(app, &release_dir, &version) {
        return Err("releaseDir 必须位于桌面的 omd-<version>-release-* 目录。".to_string());
    }
    Ok((version, release_dir))
}

fn is_allowed_release_dir(app: &AppContext, release_dir: &Path, version: &str) -> bool {
    let Ok(relative) = release_dir.strip_prefix(&app.desktop_dir) else {
        return false;
    };
    if relative.as_os_str().is_empty() {
        return false;
    }
    let Some(name) = release_dir.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    name.starts_with(&format!("omd-{version}-release-"))
}

fn summarize_release(app: &AppContext, release_dir: Option<&Path>) -> Result<Value, String> {
    let Some(release_dir) = release_dir else {
        return Ok(json!({
            "ok": true,
            "releaseDir": "",
            "message": "没有找到发布目录",
            "manifests": {},
            "artifacts": [],
            "requiredArtifacts": [],
            "checksums": [],
            "state": null
        }));
    };

    let normalized = absolute_path(release_dir);
    let state = read_json_if_exists(&normalized.join("state.json"));
    let version = state
        .as_ref()
        .and_then(|value| value["version"].as_str())
        .map(ToString::to_string)
        .or_else(|| version_from_release_dir(&normalized))
        .unwrap_or_default();
    let github_manifest = read_json_if_exists(&normalized.join("github/latest.json"));
    let gitee_manifest = read_json_if_exists(&normalized.join("gitee/latest.json"));
    let checksums = read_lines_if_exists(&normalized.join("checksums.sha256"));
    let artifacts = list_artifact_summaries(&normalized)?;
    let required_artifacts = required_artifact_summaries(app, &normalized, &version);

    Ok(json!({
        "ok": true,
        "releaseDir": normalized,
        "manifests": {
            "github": manifest_summary_with_target(github_manifest.as_ref(), &version),
            "gitee": manifest_summary_with_target(gitee_manifest.as_ref(), &version)
        },
        "manifestDiff": manifest_platform_diff(github_manifest.as_ref(), gitee_manifest.as_ref()),
        "artifacts": artifacts,
        "requiredArtifacts": required_artifacts,
        "checksums": checksums.into_iter().take(80).collect::<Vec<_>>(),
        "state": state
    }))
}

fn required_artifact_summaries(app: &AppContext, release_dir: &Path, version: &str) -> Vec<Value> {
    if version.is_empty() {
        return Vec::new();
    }
    let mut rows = Vec::new();
    for target in &app.config.required_artifacts {
        let file_name = artifact_name(&target.file, version);
        let sig_name = artifact_name(&target.signature, version);
        rows.push(artifact_summary(
            release_dir,
            &target.scope,
            &file_name,
            true,
            "",
        ));
        rows.push(artifact_summary(
            release_dir,
            &target.scope,
            &sig_name,
            true,
            &file_name,
        ));
    }
    rows
}

fn artifact_summary(
    release_dir: &Path,
    scope: &str,
    name: &str,
    required: bool,
    signature_for: &str,
) -> Value {
    let file = release_dir.join(scope).join(name);
    if let Ok(info) = fs::metadata(&file) {
        json!({
            "scope": scope,
            "name": name,
            "required": required,
            "exists": true,
            "size": info.len(),
            "modifiedAt": iso_from_system_time(info.modified().unwrap_or(SystemTime::UNIX_EPOCH)),
            "signatureFor": signature_for
        })
    } else {
        json!({
            "scope": scope,
            "name": name,
            "required": required,
            "exists": false,
            "size": 0,
            "modifiedAt": "",
            "signatureFor": signature_for
        })
    }
}

fn list_artifact_summaries(release_dir: &Path) -> Result<Vec<Value>, String> {
    let mut rows = Vec::new();
    for scope in ["windows", "mac"] {
        let dir = release_dir.join(scope);
        if !dir.exists() {
            continue;
        }
        for entry in fs::read_dir(&dir).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let info = entry.metadata().map_err(|error| error.to_string())?;
            if info.is_file() {
                rows.push(json!({
                    "scope": scope,
                    "name": entry.file_name().to_string_lossy(),
                    "size": info.len(),
                    "modifiedAt": iso_from_system_time(info.modified().unwrap_or(SystemTime::UNIX_EPOCH))
                }));
            }
        }
    }
    rows.sort_by_key(|item| {
        format!(
            "{}:{}",
            item["scope"].as_str().unwrap_or(""),
            item["name"].as_str().unwrap_or("")
        )
    });
    Ok(rows)
}

fn manifest_summary(manifest: Option<&Value>) -> Value {
    let Some(manifest) = manifest else {
        return json!({ "exists": false, "version": "", "platforms": [] });
    };
    let platforms = manifest["platforms"]
        .as_object()
        .map(|items| {
            items
                .iter()
                .map(|(key, value)| {
                    json!({
                        "key": key,
                        "hasSignature": value["signature"].as_str().map(|item| !item.is_empty()).unwrap_or(false),
                        "url": value["url"].as_str().unwrap_or("")
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({
        "exists": true,
        "version": manifest["version"].as_str().unwrap_or(""),
        "notes": manifest["notes"].as_str().unwrap_or(""),
        "pubDate": manifest["pub_date"].as_str().unwrap_or(""),
        "platforms": platforms
    })
}

fn manifest_summary_with_target(manifest: Option<&Value>, target_version: &str) -> Value {
    let mut summary = manifest_summary(manifest);
    summary["versionStatus"] = json!(manifest_version_status(
        target_version,
        manifest.and_then(|value| value["version"].as_str())
    ));
    summary
}

fn manifest_version_status(target: &str, current: Option<&str>) -> &'static str {
    match current {
        Some(value) if value == target => "same_version",
        Some(_) => "different_version",
        None => "missing",
    }
}

fn manifest_platform_diff(github: Option<&Value>, gitee: Option<&Value>) -> Value {
    let github_keys: Vec<String> = github
        .and_then(|value| value["platforms"].as_object())
        .map(|items| items.keys().cloned().collect())
        .unwrap_or_default();
    let gitee_keys: Vec<String> = gitee
        .and_then(|value| value["platforms"].as_object())
        .map(|items| items.keys().cloned().collect())
        .unwrap_or_default();
    json!({
        "missingOnGithub": gitee_keys.iter().filter(|key| !github_keys.contains(key)).cloned().collect::<Vec<_>>(),
        "missingOnGitee": github_keys.iter().filter(|key| !gitee_keys.contains(key)).cloned().collect::<Vec<_>>()
    })
}

fn release_commands(version: &str) -> Value {
    json!({
        "version": version,
        "commands": [
            "cargo run -- plan",
            format!("cargo run -- preflight {version}"),
            format!("cargo run -- build-windows {version} --release-dir ~/Desktop/omd-{version}-release-YYYYMMDD-HHMMSS"),
            format!("cargo run -- build-mac {version} --release-dir ~/Desktop/omd-{version}-release-YYYYMMDD-HHMMSS"),
            format!("cargo run -- collect-windows-artifacts {version} --release-dir ~/Desktop/omd-{version}-release-YYYYMMDD-HHMMSS"),
            format!("cargo run -- check-artifacts {version} --release-dir ~/Desktop/omd-{version}-release-YYYYMMDD-HHMMSS"),
            format!("cargo run -- manifest {version} --release-dir ~/Desktop/omd-{version}-release-YYYYMMDD-HHMMSS"),
            format!("cargo run -- release {version} --release-dir ~/Desktop/omd-{version}-release-YYYYMMDD-HHMMSS --step publish-github --confirm-version {version}"),
            format!("cargo run -- release {version} --release-dir ~/Desktop/omd-{version}-release-YYYYMMDD-HHMMSS --step publish-gitee --confirm-version {version}"),
            format!("cargo run -- verify {version} --release-dir ~/Desktop/omd-{version}-release-YYYYMMDD-HHMMSS"),
            format!("cargo run -- report {version} --release-dir ~/Desktop/omd-{version}-release-YYYYMMDD-HHMMSS"),
            format!("cargo run -- release {version} --release-dir ~/Desktop/omd-{version}-release-YYYYMMDD-HHMMSS --confirm-version {version}")
        ]
    })
}

fn config_summary(config: &ReleaseConfig) -> Value {
    json!({
        "sourceRepo": config.source_repo,
        "githubRepo": config.github_repo,
        "giteeOwner": config.gitee_owner,
        "giteeRepo": config.gitee_repo,
        "giteeLatest": config.gitee_latest,
        "githubLatest": config.github_latest,
        "windowsSsh": config.windows_ssh,
        "windowsRepo": config.windows_repo,
        "windowsTarget": config.windows_target,
        "windowsBuildScript": windows_build_script(config),
        "windowsBuildDir": windows_build_dir(config),
        "windowsExportDirPattern": config
            .windows_export_dir_pattern
            .as_deref()
            .unwrap_or(r"C:\Users\4090\AppData\Local\Temp\omd-release-export-{version}"),
        "windowsBranch": windows_branch(config),
        "githubProxy": if config.github_proxy.as_deref().unwrap_or("").is_empty() { "not_configured" } else { "configured" },
        "releaseDirPattern": config.release_dir_pattern,
        "requiredArtifacts": config.required_artifacts.iter().map(|item| json!({
            "key": item.key,
            "scope": item.scope,
            "file": item.file,
            "signature": item.signature,
            "sourceFile": item.source_file,
            "sourceSignature": item.source_signature
        })).collect::<Vec<_>>()
    })
}

fn action_error(action: &str, status: &str, message: &str) -> Value {
    json!({
        "ok": false,
        "action": action,
        "status": status,
        "message": message,
        "state": null,
        "logs": { "stdout": "", "stderr": "" },
        "suggestions": [{ "severity": "error", "message": message, "action": action }]
    })
}

fn serve_static(app: &AppContext, stream: &mut TcpStream, path: &str) -> Result<(), String> {
    let safe_path = if path == "/" {
        "index.html".to_string()
    } else {
        path.trim_start_matches('/').to_string()
    };
    if safe_path.contains("..") {
        return send_text(stream, 403, "text/plain; charset=utf-8", "Forbidden");
    }
    let requested = app.public_dir.join(&safe_path);
    let target = if requested.exists() {
        requested
    } else {
        app.public_dir.join("index.html")
    };
    let body = fs::read(&target).map_err(|error| error.to_string())?;
    let mime = match target
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
    {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    };
    send_bytes(stream, 200, mime, &body)
}

fn find_latest_release_dir(desktop_dir: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(desktop_dir).ok()?;
    let mut candidates = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if !path.is_dir()
            || !name.starts_with("omd-")
            || !name.contains("-release-")
            || version_from_release_dir(&path).is_none()
        {
            continue;
        }
        let mtime = entry
            .metadata()
            .and_then(|info| info.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        candidates.push((mtime, path));
    }
    candidates.sort_by_key(|(mtime, _)| *mtime);
    candidates.pop().map(|(_, path)| path)
}

fn read_http_request(stream: &mut TcpStream) -> Result<String, String> {
    let mut buffer = [0_u8; 1024 * 1024];
    let bytes = stream
        .read(&mut buffer)
        .map_err(|error| error.to_string())?;
    Ok(String::from_utf8_lossy(&buffer[..bytes]).to_string())
}

fn parse_request_line(request: &str) -> Result<(String, String), String> {
    let line = request.lines().next().ok_or_else(|| "空请求".to_string())?;
    let mut parts = line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "请求缺少 method".to_string())?
        .to_string();
    let target = parts
        .next()
        .ok_or_else(|| "请求缺少 path".to_string())?
        .to_string();
    Ok((method, target))
}

fn split_query(target: &str) -> (String, String) {
    if let Some((path, query)) = target.split_once('?') {
        (path.to_string(), query.to_string())
    } else {
        (target.to_string(), String::new())
    }
}

fn query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (left, right) = pair.split_once('=')?;
        if percent_decode(left) == key {
            Some(percent_decode(right))
        } else {
            None
        }
    })
}

fn request_body(request: &str) -> String {
    request
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string())
        .unwrap_or_default()
}

fn send_json(stream: &mut TcpStream, status: u16, payload: &Value) -> Result<(), String> {
    let body = serde_json::to_vec_pretty(payload).map_err(|error| error.to_string())?;
    send_bytes(stream, status, "application/json; charset=utf-8", &body)
}

fn send_text(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &str,
) -> Result<(), String> {
    send_bytes(stream, status, content_type, body.as_bytes())
}

fn send_bytes(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<(), String> {
    let reason = match status {
        200 => "OK",
        403 => "Forbidden",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        _ => "OK",
    };
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(header.as_bytes())
        .map_err(|error| error.to_string())?;
    stream.write_all(body).map_err(|error| error.to_string())
}

fn fetch_json(ctx: &CliContext, url: &str) -> Result<Value, String> {
    let mut args = vec!["-L".to_string()];
    args.extend(curl_proxy(ctx, url));
    args.extend([
        "--connect-timeout".to_string(),
        "10".to_string(),
        "--max-time".to_string(),
        "30".to_string(),
        "-sS".to_string(),
        url.to_string(),
    ]);
    let arg_refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let raw = run_capture("curl", &arg_refs, None)?;
    serde_json::from_str(&raw).map_err(|error| error.to_string())
}

fn http_status(ctx: &CliContext, url: &str) -> Result<i32, String> {
    let mut args = vec!["-L".to_string()];
    args.extend(curl_proxy(ctx, url));
    args.extend([
        "-o".to_string(),
        "/dev/null".to_string(),
        "-sS".to_string(),
        "-w".to_string(),
        "%{http_code}".to_string(),
        "--connect-timeout".to_string(),
        "10".to_string(),
        "--max-time".to_string(),
        "30".to_string(),
        url.to_string(),
    ]);
    let arg_refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let raw = run_capture("curl", &arg_refs, None)?;
    raw.parse::<i32>().map_err(|error| error.to_string())
}

fn curl_proxy(ctx: &CliContext, url: &str) -> Vec<String> {
    if url.contains("github.com") {
        if let Some(proxy) = ctx
            .app
            .config
            .github_proxy
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            return vec!["--proxy".to_string(), proxy.to_string()];
        }
    }
    Vec::new()
}

fn read_json(file: &Path) -> Result<Value, String> {
    let raw = fs::read_to_string(file).map_err(|error| format!("{}: {error}", file.display()))?;
    serde_json::from_str(&raw).map_err(|error| format!("{}: {error}", file.display()))
}

fn read_json_if_exists(file: &Path) -> Option<Value> {
    if file.exists() {
        read_json(file).ok()
    } else {
        None
    }
}

fn read_lines_if_exists(file: &Path) -> Vec<String> {
    fs::read_to_string(file)
        .map(|raw| {
            raw.lines()
                .filter(|line| !line.is_empty())
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn list_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    if !dir.exists() {
        return Ok(files);
    }
    for entry in fs::read_dir(dir).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let info = entry.metadata().map_err(|error| error.to_string())?;
        if info.is_dir() {
            files.extend(list_files(&path)?);
        } else if info.is_file() {
            files.push(path);
        }
    }
    Ok(files)
}

fn sha256(file: &Path) -> Result<String, String> {
    let raw = run_capture("shasum", &["-a", "256", &file.to_string_lossy()], None)?;
    raw.split_whitespace()
        .next()
        .map(ToString::to_string)
        .ok_or_else(|| "shasum 输出为空".to_string())
}

fn has_command(command: &str) -> bool {
    Command::new("which")
        .arg(command)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn has_gitee_credential() -> bool {
    gitee_access_token().is_ok()
}

fn gitee_access_token() -> Result<String, String> {
    let mut child = match Command::new("git")
        .args(["credential", "fill"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => return Err(error.to_string()),
    };
    if let Some(stdin) = child.stdin.as_mut() {
        let _ = stdin.write_all(b"protocol=https\nhost=gitee.com\n\n");
    }
    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(error) => return Err(error.to_string()),
    };
    if !output.status.success() {
        return Err("无法从 git credential 读取 Gitee 凭据".to_string());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("password=").map(ToString::to_string))
        .filter(|token| !token.trim().is_empty())
        .ok_or_else(|| "Gitee 凭据中缺少 password/access token".to_string())
}

fn run_status(command: &str, args: &[&str], cwd: Option<&Path>) -> Result<(), String> {
    let mut cmd = Command::new(command);
    cmd.args(args);
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    let output = cmd.output().map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "命令执行失败：{} {}\n{}{}",
            command,
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

fn run_capture(command: &str, args: &[&str], cwd: Option<&Path>) -> Result<String, String> {
    let mut cmd = Command::new(command);
    cmd.args(args);
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    let output = cmd.output().map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn run_command_output(
    command: &str,
    args: &[&str],
    cwd: Option<&Path>,
) -> Result<CommandOutput, String> {
    let mut cmd = Command::new(command);
    cmd.args(args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    let mut child = cmd.spawn().map_err(|error| error.to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法读取命令 stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "无法读取命令 stderr".to_string())?;
    let stdout_thread = thread::spawn(move || forward_pipe(stdout, None, false));
    let stderr_thread = thread::spawn(move || forward_pipe(stderr, None, true));
    let status = child.wait().map_err(|error| error.to_string())?;
    let stdout = join_pipe_thread(stdout_thread)?;
    let stderr = join_pipe_thread(stderr_thread)?;
    Ok(CommandOutput {
        status: status.code().unwrap_or(1),
        stdout,
        stderr,
    })
}

fn run_command_input_output(
    command: &str,
    args: &[&str],
    input: &str,
    cwd: Option<&Path>,
) -> Result<CommandOutput, String> {
    let mut cmd = Command::new(command);
    cmd.args(args);
    cmd.stdin(Stdio::piped());
    if let Some(cwd) = cwd {
        cmd.current_dir(cwd);
    }
    let mut child = cmd.spawn().map_err(|error| error.to_string())?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin
            .write_all(input.as_bytes())
            .map_err(|error| error.to_string())?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| error.to_string())?;
    Ok(CommandOutput {
        status: output.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn run_binary(
    binary: &Path,
    args: &[String],
    cwd: &Path,
    log_file: &Path,
) -> Result<CommandOutput, String> {
    let mut child = Command::new(binary)
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法读取动作 stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "无法读取动作 stderr".to_string())?;
    let stdout_log = log_file.to_path_buf();
    let stderr_log = log_file.to_path_buf();
    let stdout_thread = thread::spawn(move || forward_pipe(stdout, Some(stdout_log), false));
    let stderr_thread = thread::spawn(move || forward_pipe(stderr, Some(stderr_log), true));
    let status = child.wait().map_err(|error| error.to_string())?;
    let stdout = join_pipe_thread(stdout_thread)?;
    let stderr = join_pipe_thread(stderr_thread)?;
    append_log_line(
        log_file,
        &format!(
            "\nfinishedAt: {}\nexitStatus: {}\n",
            iso_now(),
            status.code().unwrap_or(1)
        ),
    )?;
    Ok(CommandOutput {
        status: status.code().unwrap_or(1),
        stdout,
        stderr,
    })
}

fn forward_pipe<R: Read + Send + 'static>(
    pipe: R,
    log_file: Option<PathBuf>,
    stderr: bool,
) -> Result<String, String> {
    let mut reader = BufReader::new(pipe);
    let mut collected = String::new();
    loop {
        let mut line = Vec::new();
        let bytes = reader
            .read_until(b'\n', &mut line)
            .map_err(|error| error.to_string())?;
        if bytes == 0 {
            break;
        }
        let line = decode_process_bytes(&line);
        collected.push_str(&line);
        if stderr {
            eprint!("{line}");
        } else {
            print!("{line}");
        }
        if let Some(log_file) = log_file.as_ref() {
            append_log_line(log_file, &line)?;
        }
    }
    Ok(collected)
}

fn decode_process_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).to_string()
}

fn join_pipe_thread(handle: thread::JoinHandle<Result<String, String>>) -> Result<String, String> {
    handle
        .join()
        .map_err(|_| "日志读取线程异常退出".to_string())?
}

fn append_log_line(log_file: &Path, line: &str) -> Result<(), String> {
    if let Some(parent) = log_file.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file)
        .map_err(|error| error.to_string())?;
    file.write_all(line.as_bytes())
        .map_err(|error| error.to_string())
}

fn updater_endpoints(tauri_config: &Value) -> Vec<String> {
    tauri_config["plugins"]["updater"]["endpoints"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_flags(values: &[String]) -> BTreeMap<String, Option<String>> {
    let mut parsed = BTreeMap::new();
    let mut index = 0;
    while index < values.len() {
        let item = &values[index];
        if item.starts_with("--") {
            let key = to_camel_flag(item.trim_start_matches("--"));
            let next = values.get(index + 1);
            if let Some(next) = next.filter(|value| !value.starts_with("--")) {
                parsed.insert(key, Some(next.clone()));
                index += 2;
                continue;
            }
            parsed.insert(key, None);
        }
        index += 1;
    }
    parsed
}

fn to_camel_flag(value: &str) -> String {
    let mut output = String::new();
    let mut upper_next = false;
    for ch in value.chars() {
        if ch == '-' {
            upper_next = true;
        } else if upper_next {
            output.extend(ch.to_uppercase());
            upper_next = false;
        } else {
            output.push(ch);
        }
    }
    output
}

fn empty_step(key: &str) -> Value {
    json!({
        "key": key,
        "status": "not_started",
        "startedAt": "",
        "endedAt": "",
        "durationMs": 0,
        "message": "",
        "summary": {},
        "error": null,
        "suggestions": []
    })
}

fn ensure_array(state: &mut Value, key: &str) {
    if !state[key].is_array() {
        state[key] = json!([]);
    }
}

fn ensure_object(state: &mut Value, key: &str) {
    if !state[key].is_object() {
        state[key] = json!({});
    }
}

fn push_array(target: &mut Value, item: Value) {
    if !target.is_array() {
        *target = json!([]);
    }
    target.as_array_mut().expect("array ensured").push(item);
}

fn upsert_by_key(target: &mut Value, item: Value) {
    if !target.is_array() {
        *target = json!([]);
    }
    let key = item["key"].as_str().unwrap_or("").to_string();
    let items = target.as_array_mut().expect("array ensured");
    items.retain(|existing| existing["key"].as_str().unwrap_or("") != key);
    items.push(item);
}

fn upsert_suggestion(state: &mut Value, message: &str, severity: &str, action: &str) {
    if !state["suggestions"].is_array() {
        state["suggestions"] = json!([]);
    }
    let items = state["suggestions"].as_array_mut().expect("array ensured");
    if !items
        .iter()
        .any(|item| item["message"] == message && item["action"] == action)
    {
        items.push(json!({ "severity": severity, "message": message, "action": action }));
    }
}

fn push_history(state: &mut Value, event: &str, step_key: &str, message: &str) {
    ensure_array(state, "history");
    push_array(
        &mut state["history"],
        json!({
            "event": event,
            "step": step_key,
            "message": message,
            "at": iso_now()
        }),
    );
}

fn with_default_time(mut value: Value, key: &str) -> Value {
    if value[key].is_null() {
        value[key] = json!(iso_now());
    }
    value
}

fn ensure(condition: bool, message: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(message.to_string())
    }
}

fn progress(message: &str) {
    println!("进度：{message}");
}

fn print_help() {
    println!(
        "OMD 发布 CLI\n\n用法：\n  cargo run -- serve\n  cargo run -- plan\n  cargo run -- preflight 0.0.7\n  cargo run -- manifest 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release\n  cargo run -- verify 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release\n  cargo run -- release 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release --confirm-version 0.0.7\n  cargo run -- release 0.0.7 --dry-run"
    );
}

fn print_plan() {
    println!(
        "OMD 双端发布流程：\n1. Mac 预检源码、凭据、4090 SSH 和 updater 端点。\n2. Mac 提交并推送产品源码版本。\n3. 4090 从远端 commit 创建干净 worktree。\n4. 4090 构建并签名 NSIS、MSI、绿色 zip。\n5. Mac 用 scp 拉回 Windows 产物。\n6. Mac 本机构建 dmg 和 app.tar.gz。\n7. Mac 统一生成 checksums、GitHub latest.json、Gitee latest.json。\n8. Mac 发布 GitHub Release 并标记 latest。\n9. Mac 发布 Gitee Release 并更新根 latest.json。\n10. Mac 验证两个 updater 端点和每个下载 URL。"
    );
}

fn is_version(value: &str) -> bool {
    let parts = value.split(['-', '+']).next().unwrap_or("");
    let nums = parts.split('.').collect::<Vec<_>>();
    nums.len() == 3
        && nums
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
}

fn version_from_release_dir(release_dir: &Path) -> Option<String> {
    let name = release_dir.file_name()?.to_str()?;
    let rest = name.strip_prefix("omd-")?;
    let (version, _) = rest.split_once("-release-")?;
    if is_version(version) {
        Some(version.to_string())
    } else {
        None
    }
}

fn artifact_name(template: &str, version: &str) -> String {
    template.replace("{version}", version)
}

fn expand_home(value: &str) -> PathBuf {
    if value == "~" {
        home_dir()
    } else if let Some(rest) = value.strip_prefix("~/") {
        home_dir().join(rest)
    } else {
        PathBuf::from(value)
    }
}

fn home_dir() -> PathBuf {
    env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn app_data_dir() -> PathBuf {
    home_dir()
        .join("Library")
        .join("Application Support")
        .join("omd-release-console")
}

fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

fn timestamp() -> String {
    run_capture("date", &["+%Y%m%d-%H%M%S"], None).unwrap_or_else(|_| "19700101-000000".to_string())
}

fn iso_now() -> String {
    run_capture("date", &["-u", "+%Y-%m-%dT%H:%M:%SZ"], None)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

fn iso_from_system_time(time: SystemTime) -> String {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    run_capture(
        "date",
        &["-u", "-r", &seconds.to_string(), "+%Y-%m-%dT%H:%M:%SZ"],
        None,
    )
    .unwrap_or_else(|_| iso_now())
}

fn duration_ms(started_at: &str, ended_at: &str) -> u64 {
    let Some(start) = iso_to_epoch_seconds(started_at) else {
        return 0;
    };
    let Some(end) = iso_to_epoch_seconds(ended_at) else {
        return 0;
    };
    end.saturating_sub(start) * 1000
}

fn iso_to_epoch_seconds(value: &str) -> Option<u64> {
    if value.is_empty() {
        return None;
    }
    run_capture(
        "date",
        &["-u", "-j", "-f", "%Y-%m-%dT%H:%M:%SZ", value, "+%s"],
        None,
    )
    .ok()
    .and_then(|raw| raw.parse::<u64>().ok())
}

fn percent_decode(value: &str) -> String {
    let mut output = String::new();
    let mut chars = value.as_bytes().iter().copied();
    while let Some(ch) = chars.next() {
        match ch {
            b'+' => output.push(' '),
            b'%' => {
                let hi = chars.next().unwrap_or(b'0') as char;
                let lo = chars.next().unwrap_or(b'0') as char;
                let hex = format!("{hi}{lo}");
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    output.push(byte as char);
                }
            }
            _ => output.push(ch as char),
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> ReleaseConfig {
        serde_json::from_str(DEFAULT_RELEASE_CONFIG).unwrap()
    }

    #[test]
    fn validates_semver_like_versions() {
        assert!(is_version("0.0.7"));
        assert!(is_version("1.2.3-beta.1"));
        assert!(!is_version("1.2"));
        assert!(!is_version("v1.2.3"));
    }

    #[test]
    fn extracts_version_from_release_dir() {
        assert_eq!(
            version_from_release_dir(Path::new("/Users/glame/Desktop/omd-0.0.7-release-test")),
            Some("0.0.7".to_string())
        );
    }

    #[test]
    fn converts_flag_names_to_camel_case() {
        assert_eq!(to_camel_flag("release-dir"), "releaseDir");
        assert_eq!(to_camel_flag("dry-run"), "dryRun");
    }

    #[test]
    fn calculates_step_duration_ms_from_iso_seconds() {
        assert_eq!(
            duration_ms("2026-05-20T00:00:00Z", "2026-05-20T00:00:02Z"),
            2000
        );
    }

    #[test]
    fn default_state_contains_release_run_fields() {
        let temp = std::env::temp_dir().join(format!("omd-state-test-{}", timestamp()));
        let state = StateManager::new(
            temp.clone(),
            "0.0.7".to_string(),
            "/tmp/source".to_string(),
            "abc123".to_string(),
        )
        .unwrap();

        let value = state.load_state();

        assert_eq!(value["version"], "0.0.7");
        assert_eq!(value["overallStatus"], "not_started");
        assert!(value["runId"].as_str().unwrap().starts_with("0.0.7-"));
        assert!(value["checks"].as_array().is_some());
        assert!(value["steps"]
            .as_object()
            .unwrap()
            .contains_key("preflight"));
        assert!(value["history"].as_array().is_some());

        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn step_transitions_append_history_events() {
        let temp = std::env::temp_dir().join(format!("omd-history-test-{}", timestamp()));
        let state = StateManager::new(
            temp.clone(),
            "0.0.7".to_string(),
            "/tmp/source".to_string(),
            "abc123".to_string(),
        )
        .unwrap();

        state.start_step("preflight", "开始预检").unwrap();
        state
            .complete_step("preflight", "预检通过", json!({ "passed": 3 }))
            .unwrap();

        let value = read_json(&temp.join("state.json")).unwrap();
        let history = value["history"].as_array().unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0]["event"], "step_started");
        assert_eq!(history[1]["event"], "step_completed");

        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn action_name_maps_check_artifacts_to_artifact_command() {
        let args = action_command_args(
            "check-artifacts",
            "0.0.7",
            Path::new("/tmp/omd-0.0.7-release-x"),
        )
        .unwrap();
        assert_eq!(args[0], "check-artifacts");
        assert!(args.contains(&"--release-dir".to_string()));
    }

    #[test]
    fn action_error_has_consistent_shape() {
        let value = action_error("preflight", "invalid_input", "bad input");
        assert_eq!(value["ok"], false);
        assert_eq!(value["action"], "preflight");
        assert_eq!(value["status"], "invalid_input");
        assert!(value["logs"].is_object());
        assert_eq!(value["suggestions"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn release_report_includes_version_commit_and_steps() {
        let state = json!({
            "version": "0.0.7",
            "commit": "abc123",
            "overallStatus": "success",
            "steps": { "preflight": { "status": "success", "message": "ok" } },
            "artifacts": [],
            "manifests": {}
        });
        let report = release_report_markdown(&state);
        assert!(report.contains("# OMD 0.0.7 Release Report"));
        assert!(report.contains("abc123"));
        assert!(report.contains("preflight"));
    }

    #[test]
    fn release_history_file_uses_supplied_data_dir() {
        let root = Path::new("/tmp/omd-release-console-data");
        assert_eq!(
            release_history_file(root),
            root.join("release-history.json")
        );
    }

    #[test]
    fn release_config_falls_back_to_embedded_default() {
        let temp = std::env::temp_dir().join(format!("omd-config-missing-{}", timestamp()));
        fs::create_dir_all(&temp).unwrap();

        let config = load_release_config(&temp).unwrap();

        assert_eq!(config.github_repo, "haoziqwaszx/omd-distribution");
        assert!(!temp.join("release.config.json").exists());

        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn parse_resume_from_flag_to_camel_case() {
        let flags = parse_flags(&["--resume-from".to_string(), "verify".to_string()]);
        assert_eq!(
            flags.get("resumeFrom").and_then(|value| value.as_deref()),
            Some("verify")
        );
    }

    #[test]
    fn manifest_version_status_detects_same_version() {
        assert_eq!(
            manifest_version_status("0.0.7", Some("0.0.7")),
            "same_version"
        );
        assert_eq!(
            manifest_version_status("0.0.7", Some("0.0.6")),
            "different_version"
        );
        assert_eq!(manifest_version_status("0.0.7", None), "missing");
    }

    #[test]
    fn missing_artifact_message_names_file() {
        let artifact = json!({ "scope": "windows", "name": "OMD_0.0.7_x64-setup.exe", "exists": false, "required": true });
        assert!(missing_artifact_message(&artifact).contains("OMD_0.0.7_x64-setup.exe"));
    }

    #[test]
    fn manifest_platforms_report_missing_between_hosts() {
        let github = json!({ "platforms": { "darwin-aarch64": {}, "windows-x86_64": {} } });
        let gitee = json!({ "platforms": { "darwin-aarch64": {} } });
        let diff = manifest_platform_diff(Some(&github), Some(&gitee));
        assert_eq!(diff["missingOnGitee"][0], "windows-x86_64");
    }

    #[test]
    fn release_sequence_includes_build_collect_publish_steps() {
        assert_eq!(
            release_step_order(),
            vec![
                "preflight",
                "build-windows",
                "build-mac",
                "collect-windows-artifacts",
                "check-artifacts",
                "checksums",
                "manifest",
                "publish-github",
                "publish-gitee",
                "verify",
                "report"
            ]
        );
    }

    #[test]
    fn action_names_map_build_and_collect_commands() {
        let release_dir = Path::new("/tmp/omd-0.0.7-release-x");
        assert_eq!(
            action_command_args("build-windows", "0.0.7", release_dir).unwrap()[0],
            "build-windows"
        );
        assert_eq!(
            action_command_args("collect-windows-artifacts", "0.0.7", release_dir).unwrap()[0],
            "collect-windows-artifacts"
        );
        assert_eq!(
            action_command_args("build-mac", "0.0.7", release_dir).unwrap()[0],
            "build-mac"
        );
    }

    #[test]
    fn remote_commit_is_parsed_from_windows_build_output() {
        let output = CommandOutput {
            status: 0,
            stdout: "Already up to date.\nOMD_REMOTE_COMMIT=abc123\n".to_string(),
            stderr: String::new(),
        };

        assert_eq!(
            remote_commit_from_output(&output),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn windows_rustc_access_violation_is_retryable() {
        let output = CommandOutput {
            status: -1073741819,
            stdout: "error: could not compile `regex-syntax` (lib)".to_string(),
            stderr: "exit code: 0xc0000005, STATUS_ACCESS_VIOLATION".to_string(),
        };

        assert!(is_windows_rustc_access_violation(&output));
    }

    #[test]
    fn command_output_summary_uses_last_non_empty_line() {
        let output = CommandOutput {
            status: 1,
            stdout: "Already up to date.\nERROR: package.json version 0.0.6 does not match target 0.0.7\n".to_string(),
            stderr: "\nPreparing worktree (detached HEAD 9e71a60)\n".to_string(),
        };

        assert_eq!(
            command_output_summary(&output),
            "ERROR: package.json version 0.0.6 does not match target 0.0.7"
        );
    }

    #[test]
    fn windows_build_script_uses_wrapper_and_generates_green_artifacts() {
        let script = fs::read_to_string("scripts/windows/omd-release-build.ps1").unwrap();

        assert!(script.contains("Invoke-Native -Command 'npm' -Arguments @('run', 'tauri:build', '--', '--bundles', 'nsis,msi'"));
        assert!(script.contains(
            "Invoke-Native -Command 'npm' -Arguments @('run', 'tauri:build', '--', '--no-bundle'"
        ));
        assert!(script.contains("src-tauri\\target\\$WindowsTarget\\release\\bundle\\green"));
        assert!(script.contains("OMD_${Version}_x64_green.zip"));
        assert!(script.contains("Run-Tauri-Signer $BuildDir $greenZip $greenSig"));
        assert!(script.contains("$env:RUSTC_WRAPPER = 'sccache'"));
    }

    #[test]
    fn dry_run_commands_use_reusable_windows_script_and_export_zip() {
        let temp = std::env::temp_dir().join(format!("omd-dry-run-test-{}", timestamp()));
        let app = AppContext {
            root: PathBuf::from("/repo/root"),
            public_dir: PathBuf::from("/repo/root/public"),
            desktop_dir: PathBuf::from("/tmp"),
            data_dir: PathBuf::from("/tmp/data"),
            config: test_config(),
        };
        let state = StateManager::new(
            temp.clone(),
            "0.0.7".to_string(),
            app.config.source_repo.clone(),
            "abc123".to_string(),
        )
        .unwrap();
        let ctx = CliContext {
            app,
            version: "0.0.7".to_string(),
            release_dir: temp.clone(),
            state,
            flags: BTreeMap::new(),
        };

        let commands = dry_run_commands(&ctx).unwrap();
        let script = commands.join("\n");

        assert!(script.contains("omd-release-build.ps1"));
        assert!(script.contains("windows-artifacts.zip"));
        assert!(!script.contains("npx tauri signer sign"));
        assert!(!script.contains("--password \"\""));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn lossy_bytes_are_preserved_for_command_logs() {
        assert_eq!(decode_process_bytes(b"ok\n\xffbad"), "ok\n\u{fffd}bad");
    }

    #[test]
    fn mac_build_command_sequence_pulls_before_npm_build() {
        let commands = mac_build_command_sequence("/tmp/source-repo");

        assert_eq!(
            commands,
            vec![
                CommandSpec::new("git", vec!["pull", "--ff-only"], Some("/tmp/source-repo")),
                CommandSpec::new(
                    "npm",
                    vec!["--prefix", "/tmp/source-repo", "run", "tauri:build:mac"],
                    None
                )
            ]
        );
    }

    #[test]
    fn artifact_source_paths_expand_release_variables() {
        assert_eq!(
            render_artifact_source(
                "src-tauri/target/{windowsTarget}/release/bundle/nsis/OMD_{version}_x64-setup.exe",
                "0.0.7",
                "x86_64-pc-windows-msvc"
            ),
            "src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/OMD_0.0.7_x64-setup.exe"
        );
    }

    #[test]
    fn windows_script_invocation_passes_required_arguments_with_escaped_values() {
        let mut config = test_config();
        config.windows_build_script = Some(r"C:\Scripts\O'MD` build.ps1".to_string());
        config.windows_repo = r"C:\Repos\Dix's UI".to_string();
        config.windows_build_dir = Some(r"C:\Builds\OMD` Release".to_string());
        config.windows_branch = Some("feature/o'md`release".to_string());
        config.windows_export_dir_pattern = Some(r"C:\Exports\OMD {version}\Bob's".to_string());

        let script = windows_build_script_invocation(&config, "0.0.8", false);

        assert!(script.starts_with("& 'C:\\Scripts\\O''MD`` build.ps1'"));
        assert!(script.contains(" -Version '0.0.8'"));
        assert!(script.contains(" -Repo 'C:\\Repos\\Dix''s UI'"));
        assert!(script.contains(" -BuildDir 'C:\\Builds\\OMD`` Release'"));
        assert!(script.contains(" -Branch 'feature/o''md``release'"));
        assert!(script.contains(" -ExportDir 'C:\\Exports\\OMD 0.0.8\\Bob''s'"));
        assert!(!script.contains(" -Clean"));
    }

    #[test]
    fn windows_script_invocation_can_request_clean_retry() {
        let config = test_config();

        let script = windows_build_script_invocation(&config, "0.0.7", true);

        assert!(script.ends_with(" -Clean"));
    }

    #[test]
    fn windows_default_build_helpers_match_release_machine_paths() {
        let config = ReleaseConfig {
            windows_build_script: None,
            windows_build_dir: None,
            windows_branch: None,
            windows_export_dir_pattern: None,
            ..test_config()
        };

        assert_eq!(
            windows_build_script(&config),
            r"C:\Users\4090\Desktop\omd-release-build.ps1"
        );
        assert_eq!(
            windows_build_dir(&config),
            r"C:\Users\4090\Desktop\dix-extension-ui-release-build"
        );
        assert_eq!(windows_branch(&config), "codex/tauri-rust-migration");
        assert_eq!(
            local_windows_build_script(Path::new("/repo/root")),
            Path::new("/repo/root").join("scripts/windows/omd-release-build.ps1")
        );
    }

    #[test]
    fn windows_build_script_emits_remote_commit_after_sync_build_dir() {
        let script = fs::read_to_string("scripts/windows/omd-release-build.ps1").unwrap();

        let sync_done_index = script.find("Run-Step 'sync-build-dir'").unwrap();
        let remote_commit_index = script.find("OMD_REMOTE_COMMIT=").unwrap();
        let metadata_index = script.find("Run-Step 'sync-release-metadata'").unwrap();

        assert!(script.contains("Invoke-Native -Command 'git' -Arguments @('-C', $BuildDir, 'rev-parse', 'HEAD') -CaptureOutput"));
        assert!(sync_done_index < remote_commit_index);
        assert!(remote_commit_index < metadata_index);
    }

    #[test]
    fn windows_build_script_signs_green_zip_with_explicit_empty_password_arg() {
        let script = fs::read_to_string("scripts/windows/omd-release-build.ps1").unwrap();

        assert!(script.contains("'--private-key-path', $KeyPath, '--password=', $FilePath"));
        assert!(!script.contains("TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ''"));
    }

    #[test]
    fn windows_build_script_writes_build_summary_and_signature_without_utf8_bom() {
        let script = fs::read_to_string("scripts/windows/omd-release-build.ps1").unwrap();

        assert!(script.contains("New-Object System.Text.UTF8Encoding($false)"));
        assert!(script.contains("[System.IO.File]::WriteAllText($summaryPath"));
        assert!(script
            .contains("[System.IO.File]::WriteAllText($SignatureFile, $signerOutput, $utf8NoBom)"));
        assert!(
            !script.contains("Set-Content -Encoding utf8 -NoNewline -LiteralPath $SignatureFile")
        );
        assert!(!script.contains(
            "ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 -LiteralPath $summaryPath"
        ));
    }

    #[test]
    fn windows_export_dir_uses_target_version() {
        let mut config = test_config();
        config.windows_export_dir_pattern = Some(r"C:\Temp\omd-{version}".to_string());

        assert_eq!(windows_export_dir(&config, "0.0.8"), r"C:\Temp\omd-0.0.8");
    }

    #[test]
    fn powershell_escape_doubles_single_quotes() {
        assert_eq!(powershell_escape("C:\\A'B"), "C:\\A''B");
    }

    #[test]
    fn windows_scp_path_converts_drive_paths() {
        assert_eq!(
            windows_scp_path(r"C:\Users\4090\AppData\Local\Temp\x.zip"),
            "/C:/Users/4090/AppData/Local/Temp/x.zip"
        );
    }

    #[test]
    fn windows_scp_path_keeps_relative_paths_normalized() {
        assert_eq!(windows_scp_path(r"tmp\x.zip"), "tmp/x.zip");
    }

    #[test]
    fn unzip_to_dir_extracts_zip_contents() {
        let temp = std::env::temp_dir().join(format!("omd-unzip-test-{}", timestamp()));
        fs::create_dir_all(&temp).unwrap();
        let zip_file = temp.join("input.zip");
        let source_file = temp.join("source.txt");
        let destination = temp.join("output");
        fs::write(&source_file, "hello zip").unwrap();
        let zip_arg = zip_file.to_string_lossy().to_string();
        let source_arg = source_file.to_string_lossy().to_string();
        let create_output = run_command_output(
            "python3",
            &[
                "-c",
                "import sys, zipfile; zipfile.ZipFile(sys.argv[1], 'w').write(sys.argv[2], 'nested/source.txt')",
                zip_arg.as_str(),
                source_arg.as_str(),
            ],
            None,
        )
        .unwrap();
        assert_eq!(create_output.status, 0);

        unzip_to_dir(&zip_file, &destination).unwrap();

        assert_eq!(
            fs::read_to_string(destination.join("nested/source.txt")).unwrap(),
            "hello zip"
        );
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn unzip_to_dir_rejects_path_traversal_entries() {
        let temp = std::env::temp_dir().join(format!("omd-unzip-traversal-test-{}", timestamp()));
        fs::create_dir_all(&temp).unwrap();
        let zip_file = temp.join("input.zip");
        let destination = temp.join("output");
        let outside = temp.join("evil.txt");
        let zip_arg = zip_file.to_string_lossy().to_string();
        let create_output = run_command_output(
            "python3",
            &[
                "-c",
                "import sys, zipfile; zipfile.ZipFile(sys.argv[1], 'w').writestr('../evil.txt', 'bad')",
                zip_arg.as_str(),
            ],
            None,
        )
        .unwrap();
        assert_eq!(create_output.status, 0);

        let error = unzip_to_dir(&zip_file, &destination).unwrap_err();

        assert!(error.contains("unsafe zip entry"));
        assert!(!outside.exists());
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn publish_steps_start_enabled_for_real_release() {
        assert_eq!(empty_step("publishGithub")["status"], "not_started");
        assert_eq!(empty_step("publishGitee")["status"], "not_started");
    }

    #[test]
    fn publish_asset_paths_use_host_manifest_and_artifact_dirs() {
        let temp = std::env::temp_dir().join(format!("omd-publish-assets-{}", timestamp()));
        fs::create_dir_all(temp.join("github")).unwrap();
        fs::create_dir_all(temp.join("gitee")).unwrap();
        fs::create_dir_all(temp.join("windows")).unwrap();
        fs::create_dir_all(temp.join("mac")).unwrap();
        fs::write(temp.join("github/latest.json"), "{}").unwrap();
        fs::write(temp.join("gitee/latest.json"), "{}").unwrap();
        fs::write(temp.join("windows/OMD_0.0.7_x64-setup.exe"), "exe").unwrap();
        fs::write(temp.join("mac/OMD.app.tar.gz"), "tar").unwrap();

        let github_assets = publish_asset_paths(&temp, "github").unwrap();
        let gitee_assets = publish_asset_paths(&temp, "gitee").unwrap();

        assert!(github_assets.contains(&temp.join("github/latest.json")));
        assert!(!github_assets.contains(&temp.join("gitee/latest.json")));
        assert!(gitee_assets.contains(&temp.join("gitee/latest.json")));
        assert!(github_assets.contains(&temp.join("windows/OMD_0.0.7_x64-setup.exe")));
        assert!(github_assets.contains(&temp.join("mac/OMD.app.tar.gz")));

        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn build_actions_require_preflight_success() {
        let state = json!({ "steps": { "preflight": { "status": "not_started" } } });

        let message = missing_action_prerequisite("build-windows", &state).unwrap();

        assert!(message.contains("预检"));
    }

    #[test]
    fn collect_requires_both_platform_builds() {
        let state = json!({
            "steps": {
                "preflight": { "status": "success" },
                "buildWindows": { "status": "success" },
                "buildMac": { "status": "not_started" }
            }
        });

        let message = missing_action_prerequisite("collect-windows-artifacts", &state).unwrap();

        assert!(message.contains("Mac"));
    }

    #[test]
    fn gitee_latest_sha_only_treats_404_as_missing() {
        assert_eq!(parse_gitee_latest_sha(404, "{}").unwrap(), None);

        let unauthorized = parse_gitee_latest_sha(401, r#"{"message":"bad token"}"#).unwrap_err();

        assert!(unauthorized.contains("401"));
    }

    #[test]
    fn action_log_file_sanitizes_action_names() {
        let release_dir = Path::new("/tmp/omd-0.0.7-release-x");

        assert_eq!(
            action_log_file(release_dir, "../build windows"),
            release_dir.join("logs/---build-windows.log")
        );
    }

    #[test]
    fn github_release_args_include_fixed_assets_without_shell_globs() {
        let assets = vec![
            PathBuf::from("/tmp/release/github/latest.json"),
            PathBuf::from("/tmp/release/windows/OMD_0.0.7_x64-setup.exe"),
            PathBuf::from("/tmp/release/mac/OMD.app.tar.gz"),
        ];

        let args =
            github_release_create_args("haoziqwaszx/omd-distribution", "0.0.7", "abc123", &assets);

        assert_eq!(args[0], "release");
        assert_eq!(args[1], "create");
        assert_eq!(args[2], "v0.0.7");
        assert!(args.contains(&"/tmp/release/github/latest.json".to_string()));
        assert!(args.contains(&"--repo".to_string()));
        assert!(args.contains(&"haoziqwaszx/omd-distribution".to_string()));
        assert!(args.contains(&"--target".to_string()));
        assert!(args.contains(&"abc123".to_string()));
        assert!(!args.iter().any(|arg| arg.contains('*')));
    }

    #[test]
    fn gitee_release_payload_uses_api_fields() {
        let payload = gitee_release_payload("0.0.7", "abc123");

        assert_eq!(payload["tag_name"], "v0.0.7");
        assert_eq!(payload["name"], "OMD 0.0.7");
        assert_eq!(payload["target_commitish"], "abc123");
        assert_eq!(payload["prerelease"], false);
    }

    #[test]
    fn confirm_version_requires_exact_target_match() {
        let mut flags = BTreeMap::new();
        flags.insert("confirmVersion".to_string(), Some("0.0.7".to_string()));
        assert!(confirm_version_matches(&flags, "0.0.7"));
        assert!(!confirm_version_matches(&flags, "0.0.8"));
        flags.insert("confirmVersion".to_string(), Some(" v0.0.7 ".to_string()));
        assert!(!confirm_version_matches(&flags, "0.0.7"));
    }
}
