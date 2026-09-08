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
            "usage:\n  oh login <email> <password> [--url https://openhub.run]\n  oh login --token <token> [--url https://openhub.run]\n  oh login <token>\n  oh project create <name>\n  oh clone <project-id> [dir] [--blobless]\n  oh sync\n  oh merge [branch] [--into target_branch]"
        );
        return Ok(());
    }
    let cmd = args.remove(0);
    match cmd.as_str() {
        "help" | "--help" | "-h" => {
            eprintln!(
                "usage:\n  oh login <email> <password> [--url https://openhub.run]\n  oh login --token <token> [--url https://openhub.run]\n  oh login <token>\n  oh project create <name>\n  oh clone <project-id> [dir] [--blobless]\n  oh sync\n  oh merge [branch] [--into target_branch]"
            );
            Ok(())
        }
        "login" => login(args).await,
        "project" => project(args).await,
        "clone" => clone_project(args).await,
        "sync" => sync().await,
        "merge" => merge(args).await,
        other => bail!("unknown command {other}"),
    }
}

async fn login(args: Vec<String>) -> Result<()> {
    let mut url = "https://openhub.run".to_string();
    let mut explicit_token = None;
    let mut rest = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--url" && i + 1 < args.len() {
            url = args[i + 1].clone();
            i += 2;
        } else if (args[i] == "--token" || args[i] == "-t") && i + 1 < args.len() {
            explicit_token = Some(args[i + 1].clone());
            i += 2;
        } else {
            rest.push(args[i].clone());
            i += 1;
        }
    }

    let token = if let Some(tok) = explicit_token {
        tok
    } else if rest.len() == 1 {
        // Single positional argument treated as session / API token
        rest[0].clone()
    } else if rest.len() >= 2 {
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
        body["token"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("no token in response"))?
            .to_string()
    } else {
        bail!("oh login <email> <password> | oh login --token <TOKEN> [--url ORIGIN]");
    };

    // Verify token validity against /api/v1/me
    let origin = url.trim_end_matches('/').to_string();
    let verify_client = reqwest::Client::new();
    let verify_res = verify_client
        .get(format!("{origin}/api/v1/me"))
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
        .send()
        .await
        .context("verifying token with server")?;

    if !verify_res.status().is_success() {
        bail!(
            "token verification failed ({}) - invalid or expired token",
            verify_res.status()
        );
    }

    let me_body: Value = verify_res.json().await.unwrap_or(Value::Null);
    let username = me_body["username"]
        .as_str()
        .or_else(|| me_body["email"].as_str())
        .unwrap_or("user");

    save_creds(&Creds {
        origin: origin.clone(),
        token,
    })?;
    println!("logged in as {username} ({origin})");
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
    let mut blobless = false;
    let mut positional = Vec::new();
    for arg in args {
        if arg == "--blobless" || arg == "--lazy" {
            blobless = true;
        } else {
            positional.push(arg);
        }
    }
    if positional.is_empty() {
        bail!("oh clone <project-id> [dir] [--blobless]");
    }
    let mut identifier = positional[0].clone();
    let dir_arg = positional.get(1).cloned();
    let creds = load_creds()?;
    let client = client(&creds);

    // If identifier doesn't contain '/', query /api/v1/me to find current username or match directly
    if !identifier.contains('/') {
        let me_res = client
            .get(format!("{}/api/v1/me", creds.origin))
            .send()
            .await;
        if let Ok(res) = me_res {
            if res.status().is_success() {
                let body: Value = res.json().await.unwrap_or(Value::Null);
                if let Some(username) = body["username"].as_str() {
                    // Try to resolve as username/identifier first
                    let candidate = format!("{username}/{identifier}");
                    let check = client
                        .get(format!("{}/api/v1/projects/{candidate}", creds.origin))
                        .send()
                        .await;
                    if let Ok(c_res) = check {
                        if c_res.status().is_success() {
                            identifier = candidate;
                        }
                    }
                }
            }
        }
    }

    let dir = PathBuf::from(dir_arg.unwrap_or_else(|| {
        identifier
            .split('/')
            .next_back()
            .unwrap_or(&identifier)
            .to_string()
    }));

    // Resolve canonical project id from server (supports UUID, slug, or username/repo)
    let project_res = client
        .get(format!("{}/api/v1/projects/{identifier}", creds.origin))
        .send()
        .await;
    let mut project_id = match project_res {
        Ok(res) if res.status().is_success() => {
            let body: Value = res.json().await.unwrap_or(Value::Null);
            body["id"].as_str().unwrap_or(&identifier).to_string()
        }
        _ => identifier.clone(),
    };

    // If still looks like username/slug or slug, try finding it in /api/v1/projects list
    if project_id == identifier {
        let list_res = client
            .get(format!("{}/api/v1/projects", creds.origin))
            .send()
            .await;
        if let Ok(res) = list_res {
            if res.status().is_success() {
                let body: Value = res.json().await.unwrap_or(Value::Null);
                if let Some(list) = body["projects"].as_array() {
                    for p in list {
                        let id_match = p["id"].as_str() == Some(&identifier);
                        let slug_match = p["slug"].as_str() == Some(&identifier);
                        let full_match = p["full_name"].as_str() == Some(&identifier);
                        let combo_match = identifier
                            .split_once('/')
                            .map(|(_, s)| p["slug"].as_str() == Some(s))
                            .unwrap_or(false);
                        if id_match || slug_match || full_match || combo_match {
                            if let Some(real_id) = p["id"].as_str() {
                                project_id = real_id.to_string();
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    let git_url = format!("{}/git/{identifier}", creds.origin);
    let header = format!("http.extraHeader=Authorization: Bearer {}", creds.token);
    let mut clone_cmd = Command::new("git");
    clone_cmd.args(["-c", &header, "clone"]);
    if blobless {
        clone_cmd.args(["--filter=blob:none"]);
    }
    clone_cmd.args([&git_url, dir.to_str().unwrap_or(".")]);
    let cloned = clone_cmd
        .status()
        .ok()
        .map(|s| s.success())
        .unwrap_or(false);
    if cloned {
        write_repo_meta(
            &dir,
            &RepoMeta {
                project_id: project_id.clone(),
                origin: creds.origin.clone(),
            },
        )?;
        prefetch_critical_files(&dir);
        pull_events(&client, &creds, &project_id, &dir).await?;
        println!("cloned {identifier} -> {}", dir.display());
        return Ok(());
    }
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
        // Auto-commit any unstaged or untracked changes before pushing
        let has_changes = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&dir)
            .output()
            .ok()
            .map(|o| !o.stdout.is_empty())
            .unwrap_or(false);

        if has_changes {
            let _ = Command::new("git")
                .args(["add", "-A"])
                .current_dir(&dir)
                .status();

            let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
            let commit_msg = format!("sync: snapshot {timestamp}");
            let committed = Command::new("git")
                .args(["commit", "-m", &commit_msg])
                .current_dir(&dir)
                .status()
                .ok()
                .map(|s| s.success())
                .unwrap_or(false);

            if committed {
                println!("created snapshot commit: {commit_msg}");
            }
        }

        let git_url = format!("{}/git/{}", creds.origin, meta.project_id);
        let auth_header = format!("http.extraHeader=Authorization: Bearer {}", creds.token);

        // 1. Push local branches to remote via Git Smart HTTP with auth header
        let _ = Command::new("git")
            .args(["-c", &auth_header, "push", &git_url, "HEAD:refs/heads/main"])
            .current_dir(&dir)
            .status();

        // 2. Fetch any remote updates without forcing into currently checked out branch
        let fetched = Command::new("git")
            .args(["-c", &auth_header, "fetch", &git_url])
            .current_dir(&dir)
            .status()
            .ok()
            .map(|s| s.success())
            .unwrap_or(false);

        if fetched {
            prefetch_critical_files(&dir);
        }
    }
    push_local_events(&client, &creds, &meta.project_id, &dir).await?;
    pull_events(&client, &creds, &meta.project_id, &dir).await?;
    println!("synced {}", meta.project_id);
    Ok(())
}

async fn merge(args: Vec<String>) -> Result<()> {
    let mut branch = None;
    let mut into = "main".to_string();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--into" | "-i" => {
                i += 1;
                if i < args.len() {
                    into = args[i].clone();
                } else {
                    bail!("missing target branch after --into");
                }
            }
            arg if !arg.starts_with('-') && branch.is_none() => {
                branch = Some(arg.to_string());
            }
            other => bail!("unexpected argument for merge: {other}"),
        }
        i += 1;
    }

    let dir = std::env::current_dir()?;
    if !dir.join(".git").exists() {
        bail!("not a git repository (run inside an OpenHub project)");
    }

    // Determine current branch
    let current_branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&dir)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "main".to_string());

    let (source_branch, target_branch) = match branch {
        Some(b) => (b, into),
        None => {
            // When no branch specified:
            if current_branch != "main" {
                // If on a feature branch, merge current branch into main
                (current_branch.clone(), "main".to_string())
            } else {
                // Already on main, fetch and merge latest remote main
                ("origin/main".to_string(), "main".to_string())
            }
        }
    };

    let meta = load_repo_meta(&dir)?;
    let creds = load_creds()?;
    let client = client(&creds);

    // 1. Check working directory status
    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&dir)
        .output()
        .ok()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);

    if dirty {
        println!("creating snapshot of uncommitted changes before merge...");
        let _ = Command::new("git")
            .args(["add", "-A"])
            .current_dir(&dir)
            .status();

        let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let commit_msg = format!("sync: snapshot before merging {source_branch} at {timestamp}");
        let _ = Command::new("git")
            .args(["commit", "-m", &commit_msg])
            .current_dir(&dir)
            .status();
    }

    println!(
        "merging '{source_branch}' into '{target_branch}' (current branch: {current_branch})..."
    );

    // 2. Switch to target branch if not already on it
    if current_branch != target_branch {
        let checkout_status = Command::new("git")
            .args(["checkout", &target_branch])
            .current_dir(&dir)
            .status()
            .context(format!("switching to target branch {target_branch}"))?;

        if !checkout_status.success() {
            bail!("failed to checkout target branch '{target_branch}'");
        }
    }

    // 3. If source is remote (e.g. origin/main), fetch first
    if source_branch.starts_with("origin/") {
        let git_url = format!("{}/git/{}", creds.origin, meta.project_id);
        let auth_header = format!("http.extraHeader=Authorization: Bearer {}", creds.token);
        println!("fetching latest updates from remote...");
        let _ = Command::new("git")
            .args(["-c", &auth_header, "fetch", &git_url])
            .current_dir(&dir)
            .status();
    }

    // 4. Perform git merge
    let merge_status = Command::new("git")
        .args([
            "merge",
            &source_branch,
            "--no-edit",
            "-m",
            &format!("merge: integrate {source_branch} into {target_branch}"),
        ])
        .current_dir(&dir)
        .status()
        .context(format!("merging {source_branch}"))?;

    if !merge_status.success() {
        bail!(
            "merge encountered conflicts or errors. Resolve conflicts in working tree, then run 'oh sync'."
        );
    }

    println!("successfully merged '{source_branch}' into '{target_branch}'.");

    // 5. Push remote and sync session events
    let git_url = format!("{}/git/{}", creds.origin, meta.project_id);
    let auth_header = format!("http.extraHeader=Authorization: Bearer {}", creds.token);
    let refspec = format!("HEAD:refs/heads/{target_branch}");
    println!("pushing merged {target_branch} to remote...");
    let push_status = Command::new("git")
        .args(["-c", &auth_header, "push", &git_url, &refspec])
        .current_dir(&dir)
        .status()
        .context("pushing merged branch")?;

    if !push_status.success() {
        eprintln!("warning: push to remote returned non-zero status");
    }

    // 6. Push local agent sessions and pull events
    push_local_events(&client, &creds, &meta.project_id, &dir).await?;
    pull_events(&client, &creds, &meta.project_id, &dir).await?;

    println!(
        "merged and synced {} ({} -> {})",
        meta.project_id, source_branch, target_branch
    );
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
    let mut all_events = Vec::new();

    // 1. Read existing OpenHub local events from .openhub/events.json
    let local = read_local_events(dir)?;
    if let Some(arr) = local.as_array() {
        all_events.extend(arr.clone());
    }

    // 2. Discover and collect agent sessions (starting with .pi coding agent)
    let pi_events = collect_pi_sessions(dir);
    if !pi_events.is_empty() {
        println!("found {} .pi session event(s) to sync", pi_events.len());
        all_events.extend(pi_events);
    }

    if all_events.is_empty() {
        return Ok(());
    }

    let mut total_accepted = 0;
    let mut total_skipped = 0;

    // Send in chunks of 50-100 events to avoid HTTP 413 Payload Too Large
    for chunk in all_events.chunks(100) {
        let res = client
            .post(format!(
                "{}/api/v1/projects/{project_id}/events",
                creds.origin
            ))
            .json(&json!({ "events": chunk }))
            .send()
            .await
            .context("push events")?;

        if !res.status().is_success() {
            let status = res.status();
            let err_text = res.text().await.unwrap_or_default();
            eprintln!("session push chunk {status}: {err_text}");
        } else {
            let resp: Value = res.json().await.unwrap_or(Value::Null);
            total_accepted += resp["accepted"].as_u64().unwrap_or(0);
            total_skipped += resp["skipped"].as_u64().unwrap_or(0);
        }
    }

    println!("synced events: {total_accepted} new, {total_skipped} existing");
    Ok(())
}

/// Collects .pi coding agent session events from ~/.pi/agent/sessions/--<path>--/*.jsonl
fn collect_pi_sessions(dir: &Path) -> Vec<Value> {
    let canonical = match dir.canonicalize() {
        Ok(p) => p,
        Err(_) => dir.to_path_buf(),
    };
    let home = match std::env::var("HOME") {
        Ok(h) => PathBuf::from(h),
        Err(_) => return Vec::new(),
    };

    // Calculate commit sha if in a git repo to link session events with commit
    let commit_sha = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&canonical)
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                String::from_utf8(out.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        });

    // .pi encodes path "/Users/xinference/github/openhub" into "--Users-xinference-github-openhub--"
    // Check both canonicalized path and non-canonicalized path (e.g. macOS /var -> /private/var, /tmp -> /private/tmp)
    let candidates = [
        canonical.to_string_lossy().to_string(),
        dir.to_string_lossy().to_string(),
    ];

    let mut sessions_dir = None;
    for cand in candidates {
        let safe = format!("--{}--", cand.trim_matches('/').replace('/', "-"));
        let candidate_dir = home.join(".pi").join("agent").join("sessions").join(safe);
        if candidate_dir.is_dir() {
            sessions_dir = Some(candidate_dir);
            break;
        }
    }

    let sessions_dir = match sessions_dir {
        Some(d) => d,
        None => return Vec::new(),
    };

    let entries = match fs::read_dir(&sessions_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut events = Vec::new();
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|ext| ext.to_str()) == Some("jsonl"))
        .collect();
    files.sort();

    for file in files {
        let content = match fs::read_to_string(&file) {
            Ok(c) => c,
            Err(_) => continue,
        };

        for (idx, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let mut val: Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let ev_type_raw = val
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("unknown");
            let event_type = format!("pi.{ev_type_raw}");

            let id = val
                .get("id")
                .and_then(|i| i.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    let fname = file
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("session");
                    format!("pi-{fname}-{idx}")
                });

            let created_at = val
                .get("timestamp")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

            // Augment payload with commit_sha if available
            if let Some(ref sha) = commit_sha {
                if let Some(obj) = val.as_object_mut() {
                    obj.insert("commit_sha".to_string(), json!(sha));
                }
            }

            events.push(json!({
                "id": id,
                "event_type": event_type,
                "payload": val,
                "created_at": created_at,
            }));
        }
    }

    events
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

/// Pre-hydrates key project descriptors and agent directives into the working tree cache.
/// Inspired by Cloudflare ArtifactFS priority hydration, ensuring agents can inspect
/// structural files immediately upon workspace initialization.
fn prefetch_critical_files(dir: &Path) {
    const CRITICAL_FILES: &[&str] = &[
        "AGENTS.md",
        "README.md",
        "Cargo.toml",
        "package.json",
        "go.mod",
        "pyproject.toml",
        "requirements.txt",
    ];

    for file in CRITICAL_FILES {
        let path = dir.join(file);
        if path.exists() {
            // Touch/read file to trigger hydration if backed by partial/promisor remotes
            let _ = fs::read(path);
        } else {
            // Check out from HEAD silently if tree knows the file but blob wasn't hydrated
            let _ = Command::new("git")
                .args(["checkout", "HEAD", "--", file])
                .current_dir(dir)
                .stderr(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .status();
        }
    }
}
