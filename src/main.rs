use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const STEP_KEYS: [&str; 9] = [
    "preflight",
    "artifactCheck",
    "checksums",
    "manifestGithub",
    "manifestGitee",
    "dryRunRelease",
    "verify",
    "publishGithub",
    "publishGitee",
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
    github_proxy: Option<String>,
    release_dir_pattern: String,
    required_artifacts: Vec<RequiredArtifact>,
}

#[derive(Clone, Debug, Deserialize)]
struct RequiredArtifact {
    key: String,
    scope: String,
    file: String,
    signature: String,
}

#[derive(Clone)]
struct AppContext {
    root: PathBuf,
    public_dir: PathBuf,
    desktop_dir: PathBuf,
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

fn main() {
    if let Err(error) = run_main() {
        eprintln!("错误：{error}");
        std::process::exit(1);
    }
}

fn run_main() -> Result<(), String> {
    let app = AppContext::load()?;
    let args: Vec<String> = env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("serve");

    match command {
        "serve" | "server" | "dev" => serve(app),
        "plan" => {
            print_plan();
            Ok(())
        }
        "preflight" | "manifest" | "verify" | "release" => run_cli_command(app, &args),
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
        let config = load_release_config(&root)?;
        Ok(Self {
            root,
            public_dir,
            desktop_dir,
            config,
        })
    }
}

fn load_release_config(root: &Path) -> Result<ReleaseConfig, String> {
    let config_file = root.join("release.config.json");
    let raw = fs::read_to_string(&config_file)
        .map_err(|error| format!("读取 release.config.json 失败：{error}"))?;
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
    state.set_step_disabled("publishGithub", "真实发布在 Phase 1 未启用")?;
    state.set_step_disabled("publishGitee", "真实发布在 Phase 1 未启用")?;

    let ctx = CliContext {
        app,
        version,
        release_dir,
        state,
        flags,
    };

    match command {
        "preflight" => preflight(&ctx),
        "manifest" => {
            checksums(&ctx)?;
            manifests(&ctx)
        }
        "verify" => verify(&ctx),
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
                "checks": [],
                "steps": {},
                "artifacts": [],
                "manifests": {},
                "errors": [],
                "suggestions": []
            })
        };

        state["version"] = json!(state["version"].as_str().unwrap_or(&self.version));
        state["releaseDir"] = json!(state["releaseDir"]
            .as_str()
            .unwrap_or(&self.release_dir.to_string_lossy()));
        state["sourceRepo"] = json!(state["sourceRepo"].as_str().unwrap_or(&self.source_repo));
        state["commit"] = json!(state["commit"].as_str().unwrap_or(&self.commit));
        ensure_array(&mut state, "checks");
        ensure_array(&mut state, "artifacts");
        ensure_object(&mut state, "manifests");
        ensure_array(&mut state, "errors");
        ensure_array(&mut state, "suggestions");
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

    fn set_step_disabled(&self, step_key: &str, message: &str) -> Result<(), String> {
        let mut state = self.load_state();
        let mut step = state["steps"][step_key].clone();
        step["status"] = json!("disabled");
        step["message"] = json!(message);
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

    fn manual_required_step(
        &self,
        step_key: &str,
        message: &str,
        summary: Value,
        suggestion: &str,
    ) -> Result<(), String> {
        let mut state = self.load_state();
        let now = iso_now();
        let started_at = state["steps"][step_key]["startedAt"]
            .as_str()
            .unwrap_or("")
            .to_string();
        state["currentStep"] = json!(step_key);
        state["overallStatus"] = json!("failed");
        upsert_suggestion(&mut state, suggestion, "warning", step_key);
        let mut step = state["steps"][step_key].clone();
        step["status"] = json!("manual_required");
        step["endedAt"] = json!(now.clone());
        step["durationMs"] = json!(duration_ms(&started_at, &now));
        step["message"] = json!(message);
        step["summary"] = summary;
        step["error"] = Value::Null;
        step["suggestions"] = json!([suggestion]);
        state["steps"][step_key] = step;
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

    preflight(ctx)?;
    fs::create_dir_all(&ctx.release_dir).map_err(|error| error.to_string())?;
    progress(&format!("发布目录：{}", ctx.release_dir.display()));

    if ctx.flags.contains_key("dryRun") {
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

    ctx.state.fail_step(
        "dryRunRelease",
        "真实发布在 Phase 1 未启用",
        "请先使用 --dry-run 核对命令；真实发布将在后续阶段启用",
    )?;
    Err("当前版本为安全第一版：请先使用 --dry-run 核对命令，再逐步接入真实执行。".to_string())
}

fn publish_step(ctx: &CliContext, step: &str) -> Result<(), String> {
    let step_key = match step {
        "publish-github" | "publishGithub" => "publishGithub",
        "publish-gitee" | "publishGitee" => "publishGitee",
        _ => return Err(format!("未知发布步骤：{step}")),
    };
    let confirm = ctx
        .flags
        .get("confirmVersion")
        .and_then(|value| value.as_deref())
        .unwrap_or("");
    if confirm != ctx.version {
        ctx.state.fail_step(
            step_key,
            "版本确认不匹配",
            "请输入与目标版本完全一致的 confirmVersion 后再继续",
        )?;
        return Err("版本确认不匹配，已阻止真实发布步骤。".to_string());
    }

    ctx.state.start_step(step_key, "准备分段发布命令")?;
    let commands = publish_commands(ctx, step_key);
    let message = "已通过版本确认；真实上传保持人工执行";
    let suggestion =
        "请核对命令、仓库、tag、manifest 和产物数量后手动执行；自动真实上传将在后续阶段单独启用";
    ctx.state.manual_required_step(
        step_key,
        message,
        json!({
            "host": if step_key == "publishGithub" { "github" } else { "gitee" },
            "confirmVersion": confirm,
            "commands": commands
        }),
        suggestion,
    )?;
    println!("进度：{message}");
    for command in commands {
        println!("[需人工确认] {command}");
    }
    Err("真实上传仍需人工执行，Rust 后端已记录 manual_required 状态。".to_string())
}

fn dry_run_commands(ctx: &CliContext) -> Result<Vec<String>, String> {
    let commit = run_capture(
        "git",
        &["rev-parse", "HEAD"],
        Some(Path::new(&ctx.app.config.source_repo)),
    )?;
    let worktree = format!(
        "C:\\Users\\4090\\Desktop\\dix-extension-ui-release-{}-{}",
        ctx.version,
        timestamp()
    );
    let release_dir = ctx.release_dir.to_string_lossy();
    Ok(vec![
        format!("ssh {} \"powershell -NoProfile -NonInteractive -Command \\\"git -C {} fetch --prune origin; git -C {} worktree add --detach {} {}\\\"\"", ctx.app.config.windows_ssh, ctx.app.config.windows_repo, ctx.app.config.windows_repo, worktree, commit),
        format!("ssh {} \"powershell -NoProfile -NonInteractive -Command \\\"cd {}; npm ci; npm run lint; npm run build; npx tauri build --bundles nsis,msi --target {}\\\"\"", ctx.app.config.windows_ssh, worktree, ctx.app.config.windows_target),
        format!("scp {}:/C:/Users/4090/Desktop/.../OMD_{}_x64-setup.exe {release_dir}/windows/", ctx.app.config.windows_ssh, ctx.version),
        format!("npm --prefix {} run tauri:build:mac", ctx.app.config.source_repo),
        format!("cargo run -- manifest {} --release-dir {release_dir}", ctx.version),
        format!("gh release create v{} {release_dir}/github/latest.json {release_dir}/windows/* {release_dir}/mac/* -R {} --title \"OMD {}\"", ctx.version, ctx.app.config.github_repo, ctx.version),
        format!("cargo run -- verify {} --release-dir {release_dir}", ctx.version),
    ])
}

fn publish_commands(ctx: &CliContext, step_key: &str) -> Vec<String> {
    let release_dir = ctx.release_dir.to_string_lossy();
    if step_key == "publishGithub" {
        vec![format!(
            "gh release create v{} {release_dir}/github/latest.json {release_dir}/windows/* {release_dir}/mac/* -R {} --title \"OMD {}\"",
            ctx.version, ctx.app.config.github_repo, ctx.version
        )]
    } else {
        vec![
            format!("请在 Gitee 仓库 {}/{} 创建 v{} Release，并上传 {release_dir}/gitee/latest.json、windows/*、mac/*", ctx.app.config.gitee_owner, ctx.app.config.gitee_repo, ctx.version),
            format!("确认 Gitee latest.json 端点更新：{}", ctx.app.config.gitee_latest),
        ]
    }
}

fn checksums(ctx: &CliContext) -> Result<(), String> {
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
        ctx.state.fail_step(
            "artifactCheck",
            &message,
            "请补齐缺失产物后重新生成 manifest",
        )?;
        return Err(message);
    }
    ctx.state.complete_step(
        "artifactCheck",
        "产物完整",
        json!({ "count": artifacts.len() }),
    )?;

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

fn run_action(app: &AppContext, action: &str, body: &Value) -> Result<Value, String> {
    let (version, release_dir) = match validate_action_input(app, body) {
        Ok(value) => value,
        Err(message) => return Ok(action_error(action, "invalid_input", &message)),
    };

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

    let command_args = match action {
        "preflight" => vec![
            "preflight".to_string(),
            version.clone(),
            "--release-dir".to_string(),
            release_dir.to_string_lossy().to_string(),
        ],
        "manifest" => vec![
            "manifest".to_string(),
            version.clone(),
            "--release-dir".to_string(),
            release_dir.to_string_lossy().to_string(),
        ],
        "verify" => vec![
            "verify".to_string(),
            version.clone(),
            "--release-dir".to_string(),
            release_dir.to_string_lossy().to_string(),
        ],
        "dry-run-release" => vec![
            "release".to_string(),
            version.clone(),
            "--release-dir".to_string(),
            release_dir.to_string_lossy().to_string(),
            "--dry-run".to_string(),
        ],
        "publish-github" => vec![
            "release".to_string(),
            version.clone(),
            "--release-dir".to_string(),
            release_dir.to_string_lossy().to_string(),
            "--step".to_string(),
            "publish-github".to_string(),
            "--confirm-version".to_string(),
            version.clone(),
        ],
        "publish-gitee" => vec![
            "release".to_string(),
            version.clone(),
            "--release-dir".to_string(),
            release_dir.to_string_lossy().to_string(),
            "--step".to_string(),
            "publish-gitee".to_string(),
            "--confirm-version".to_string(),
            version.clone(),
        ],
        _ => {
            return Ok(action_error(
                action,
                "unknown_action",
                &format!("未知 action：{action}"),
            ))
        }
    };
    let binary = env::current_exe().map_err(|error| error.to_string())?;
    let output = run_binary(&binary, &command_args, &app.root)?;
    let state = read_json_if_exists(&release_dir.join("state.json"));
    let manual_required =
        output.stderr.contains("真实上传仍需人工执行") || output.stdout.contains("[需人工确认]");
    let ok = output.status == 0;
    Ok(json!({
        "ok": ok,
        "action": action,
        "status": if ok { "success" } else if manual_required { "manual_required" } else { "failed" },
        "message": if ok { format!("{action} 完成") } else if manual_required { format!("{action} 需要人工执行") } else { format!("{action} 失败") },
        "state": state,
        "logs": { "stdout": output.stdout, "stderr": output.stderr },
        "suggestions": state.as_ref().and_then(|item| item["suggestions"].as_array()).cloned().unwrap_or_default()
    }))
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
            "github": manifest_summary(github_manifest.as_ref()),
            "gitee": manifest_summary(gitee_manifest.as_ref())
        },
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

fn release_commands(version: &str) -> Value {
    json!({
        "version": version,
        "commands": [
            "cargo run -- plan",
            format!("cargo run -- preflight {version}"),
            format!("cargo run -- release {version} --dry-run"),
            format!("cargo run -- release {version}"),
            format!("cargo run -- verify {version} --release-dir ~/Desktop/omd-{version}-release-YYYYMMDD-HHMMSS")
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
        "githubProxy": if config.github_proxy.as_deref().unwrap_or("").is_empty() { "not_configured" } else { "configured" },
        "releaseDirPattern": config.release_dir_pattern,
        "requiredArtifacts": config.required_artifacts.iter().map(|item| json!({
            "key": item.key,
            "scope": item.scope,
            "file": item.file,
            "signature": item.signature
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
    let mut child = match Command::new("git")
        .args(["credential", "fill"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return false,
    };
    if let Some(stdin) = child.stdin.as_mut() {
        let _ = stdin.write_all(b"protocol=https\nhost=gitee.com\n\n");
    }
    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(_) => return false,
    };
    output.status.success() && String::from_utf8_lossy(&output.stdout).contains("password=")
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

fn run_binary(binary: &Path, args: &[String], cwd: &Path) -> Result<CommandOutput, String> {
    let output = Command::new(binary)
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|error| error.to_string())?;
    Ok(CommandOutput {
        status: output.status.code().unwrap_or(1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
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
    let disabled = key == "publishGithub" || key == "publishGitee";
    json!({
        "key": key,
        "status": if disabled { "disabled" } else { "not_started" },
        "startedAt": "",
        "endedAt": "",
        "durationMs": 0,
        "message": if disabled { "真实发布在 Phase 1 未启用" } else { "" },
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
        "OMD 发布 CLI\n\n用法：\n  cargo run -- serve\n  cargo run -- plan\n  cargo run -- preflight 0.0.7\n  cargo run -- manifest 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release\n  cargo run -- verify 0.0.7 --release-dir ~/Desktop/omd-0.0.7-release\n  cargo run -- release 0.0.7 --dry-run"
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
}
