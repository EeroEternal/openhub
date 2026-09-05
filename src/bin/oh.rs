//! OpenHub CLI: login, project, clone, sync (git bundle + cellz events).
#![allow(clippy::collapsible_if)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Creds {
    origin: String,
    token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RepoMeta {
    project_id: String,
    origin: String,
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("oh: {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        eprintln!(
            "usage:\n  oh login <email> <password> [--url https://openhub.run]\n  oh project create <name>\n  oh clone <project-id> [dir]\n  oh sync"
        );
        return Ok(());
    }
    let cmd = args.remove(0);
    match cmd.as_str() {
        "login" => login(args).await,
        "project" => project(args).await,
        "clone" => clone_project(args).await,
        "sync" => sync().await,
        other => bail!("unknown command {other}"),
    }
}

async fn login(args: Vec<String>) -> Result<()> {
    let mut url = "https://openhub.run".to_string();
    let mut rest = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--url" && i + 1 < args.len() {
            url = args[i + 1].clone();
            i += 2;
        } else {
            rest.push(args[i].clone());
            i += 1;
        }
    }
    if rest.len() < 2 {
        bail!("oh login <email> <password> [--url ORIGIN]");
    }
    let email = &rest[0];
    let password = &rest[1];
    let client = reqwest::Client::new();
    let res = client
        .post(format!("{}/api/v1/auth/login", url.trim_end_matches('/')))
        .json(&json!({ "email": email, "password": password }))
        .send()
        .await
        .context("login request")?;
    let status = res.status();
    let body: Value = res.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        bail!("login failed: {status} {body}");
    }
    let token = body["token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("no token in response"))?;
    save_creds(&Creds {
        origin: url.trim_end_matches('/').to_string(),
        token: token.to_string(),
    })?;
    println!("logged in");
    Ok(())
}

async fn project(args: Vec<String>) -> Result<()> {
    if args.first().map(String::as_str) != Some("create") || args.len() < 2 {
        bail!("oh project create <name>");
    }
    let name = args[1..].join(" ");
    let creds = load_creds()?;
    let client = client(&creds);
    let res = client
        .post(format!("{}/api/v1/projects", creds.origin))
        .json(&json!({ "name": name }))
        .send()
        .await
        .context("create project")?;
    let status = res.status();
    let body: Value = res.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        bail!("create failed: {status} {body}");
    }
    println!("{}", body["id"].as_str().unwrap_or(""));
    println!("slug={}", body["slug"].as_str().unwrap_or(""));
    Ok(())
}

async fn clone_project(args: Vec<String>) -> Result<()> {
    if args.is_empty() {
        bail!("oh clone <project-id> [dir]");
    }
    let project_id = args[0].clone();
    let dir = PathBuf::from(args.get(1).cloned().unwrap_or_else(|| project_id.clone()));
    let creds = load_creds()?;
    let client = client(&creds);
    let bundle = client
        .get(format!(
            "{}/api/v1/projects/{project_id}/git/bundle",
            creds.origin
        ))
        .send()
        .await
        .context("download bundle")?;
    fs::create_dir_all(&dir)?;
    if bundle.status().is_success() {
        let bytes = bundle.bytes().await?;
        if !bytes.is_empty() {
            let tmp = dir.join(".openhub-clone.bundle");
            fs::write(&tmp, &bytes)?;
            let status = Command::new("git")
                .args(["clone", tmp.to_str().unwrap(), dir.to_str().unwrap()])
                .status()
                .context("git clone bundle")?;
            let _ = fs::remove_file(&tmp);
            if !status.success() {
                init_empty(&dir)?;
            }
        } else {
            init_empty(&dir)?;
        }
    } else {
        init_empty(&dir)?;
    }
    write_repo_meta(
        &dir,
        &RepoMeta {
            project_id: project_id.clone(),
            origin: creds.origin.clone(),
        },
    )?;
    pull_events(&client, &creds, &project_id, &dir).await?;
    println!("cloned {project_id} -> {}", dir.display());
    Ok(())
}

async fn sync() -> Result<()> {
    let dir = std::env::current_dir()?;
    let meta = load_repo_meta(&dir)?;
    let creds = load_creds()?;
    if creds.origin != meta.origin {
        eprintln!(
            "warning: credentials origin {} != repo origin {}",
            creds.origin, meta.origin
        );
    }
    let client = client(&creds);
    if dir.join(".git").exists() {
        if let Ok(bytes) = create_local_bundle(&dir) {
            let res = client
                .put(format!(
                    "{}/api/v1/projects/{}/git/bundle",
                    creds.origin, meta.project_id
                ))
                .header("content-type", "application/vnd.git-bundle")
                .body(bytes)
                .send()
                .await
                .context("upload bundle")?;
            if !res.status().is_success() {
                eprintln!("git push bundle: {}", res.status());
            }
        }
        if let Ok(res) = client
            .get(format!(
                "{}/api/v1/projects/{}/git/bundle",
                creds.origin, meta.project_id
            ))
            .send()
            .await
        {
            if res.status().is_success() {
                if let Ok(bytes) = res.bytes().await {
                    let tmp = dir.join(".openhub-pull.bundle");
                    if fs::write(&tmp, &bytes).is_ok() {
                        let _ = Command::new("git")
                            .args(["fetch", tmp.to_str().unwrap(), "+refs/heads/*:refs/heads/*"])
                            .current_dir(&dir)
                            .status();
                        let _ = fs::remove_file(&tmp);
                    }
                }
            }
        }
    }
    push_local_events(&client, &creds, &meta.project_id, &dir).await?;
    pull_events(&client, &creds, &meta.project_id, &dir).await?;
    println!("synced {}", meta.project_id);
    Ok(())
}

async fn pull_events(
    client: &reqwest::Client,
    creds: &Creds,
    project_id: &str,
    dir: &Path,
) -> Result<()> {
    let res = client
        .get(format!(
            "{}/api/v1/projects/{project_id}/events",
            creds.origin
        ))
        .send()
        .await
        .context("pull events")?;
    if res.status().is_success() {
        let body: Value = res.json().await.unwrap_or(Value::Null);
        write_local_events(dir, &body["events"])?;
    }
    Ok(())
}

async fn push_local_events(
    client: &reqwest::Client,
    creds: &Creds,
    project_id: &str,
    dir: &Path,
) -> Result<()> {
    let events = read_local_events(dir)?;
    if events.as_array().map(|a| a.is_empty()).unwrap_or(true) {
        return Ok(());
    }
    let res = client
        .post(format!(
            "{}/api/v1/projects/{project_id}/events",
            creds.origin
        ))
        .json(&json!({ "events": events }))
        .send()
        .await
        .context("push events")?;
    if !res.status().is_success() {
        eprintln!("session push: {}", res.status());
    }
    Ok(())
}

fn client(creds: &Creds) -> reqwest::Client {
    reqwest::Client::builder()
        .default_headers({
            let mut h = reqwest::header::HeaderMap::new();
            if let Ok(v) =
                reqwest::header::HeaderValue::from_str(&format!("Bearer {}", creds.token))
            {
                h.insert(reqwest::header::AUTHORIZATION, v);
            }
            h
        })
        .build()
        .expect("client")
}

fn creds_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME")?;
    Ok(PathBuf::from(home).join(".openhub").join("credentials"))
}

fn save_creds(creds: &Creds) -> Result<()> {
    let path = creds_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_vec_pretty(creds)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn load_creds() -> Result<Creds> {
    let path = creds_path()?;
    let bytes = fs::read(&path).context("not logged in (oh login)")?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn repo_meta_path(dir: &Path) -> PathBuf {
    dir.join(".openhub").join("config.json")
}

fn write_repo_meta(dir: &Path, meta: &RepoMeta) -> Result<()> {
    let path = repo_meta_path(dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(meta)?)?;
    Ok(())
}

fn load_repo_meta(dir: &Path) -> Result<RepoMeta> {
    let path = repo_meta_path(dir);
    let bytes = fs::read(&path).context("not an OpenHub checkout (.openhub/config.json)")?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn events_path(dir: &Path) -> PathBuf {
    dir.join(".openhub").join("events.json")
}

fn write_local_events(dir: &Path, events: &Value) -> Result<()> {
    let path = events_path(dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(events)?)?;
    Ok(())
}

fn read_local_events(dir: &Path) -> Result<Value> {
    let path = events_path(dir);
    if !path.exists() {
        return Ok(json!([]));
    }
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn init_empty(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir)?;
    if !dir.join(".git").exists() {
        let st = Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(dir)
            .status()
            .context("git init")?;
        if !st.success() {
            bail!("git init failed");
        }
    }
    Ok(())
}

fn create_local_bundle(dir: &Path) -> Result<Vec<u8>> {
    let bundle = dir.join(".openhub").join("out.bundle");
    if let Some(p) = bundle.parent() {
        fs::create_dir_all(p)?;
    }
    let st = Command::new("git")
        .args(["bundle", "create", bundle.to_str().unwrap(), "--all"])
        .current_dir(dir)
        .status()
        .context("git bundle")?;
    if !st.success() {
        bail!("git bundle create failed (need at least one commit?)");
    }
    let bytes = fs::read(&bundle)?;
    let _ = fs::remove_file(&bundle);
    Ok(bytes)
}
