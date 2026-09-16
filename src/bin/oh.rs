//! OpenHub CLI: login, project, clone, sync (git bundle + cellz events).
#![allow(clippy::collapsible_if)]

use std::collections::HashSet;
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
    /// Optional GitHub repo to push after OpenHub (`oh github set`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    github: Option<String>,
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
        print_usage();
        return Ok(());
    }
    let cmd = args.remove(0);
    match cmd.as_str() {
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        "login" => login(args).await,
        "project" => project(args).await,
        "init" => init_project(args).await,
        "clone" => clone_project(args).await,
        "sync" => sync().await,
        "github" => github_cmd(args),
        "merge" => merge(args).await,
        other => bail!("unknown command {other}"),
    }
}

fn print_usage() {
    eprintln!(
        "usage:\n  oh login [--url https://openhub.run]\n  oh login <email> <password> [--url https://openhub.run]\n  oh login --token <token> [--url https://openhub.run]\n  oh login <token>\n  oh project create <name>\n  oh init <project-id>\n  oh clone <project-id> [dir] [--blobless]\n  oh sync\n  oh github set [owner/repo|url]\n  oh github unset\n  oh merge [branch] [--into target_branch]"
    );
}

fn open_browser(url: &str) {
    let _ = if cfg!(target_os = "macos") {
        Command::new("open").arg(url).spawn()
    } else if cfg!(target_os = "windows") {
        Command::new("cmd").args(["/C", "start", url]).spawn()
    } else {
        Command::new("xdg-open").arg(url).spawn()
    };
}

async fn browser_login(origin: &str) -> Result<String> {
    let origin = origin.trim_end_matches('/');
    let client = reqwest::Client::new();
    let res = client
        .post(format!("{origin}/api/v1/auth/cli/start"))
        .send()
        .await
        .context("start browser login")?;
    let status = res.status();
    let body: Value = res.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        bail!("start login failed: {status} {body}");
    }
    let device_code = body["device_code"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("no device_code"))?
        .to_string();
    let uri = body["verification_uri"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("no verification_uri"))?
        .to_string();
    let expires_in = body["expires_in"].as_u64().unwrap_or(600);
    println!("Open this URL to sign in:\n  {uri}");
    open_browser(&uri);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(expires_in);
    loop {
        if std::time::Instant::now() > deadline {
            bail!("login timed out; run oh login again");
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let poll = client
            .post(format!("{origin}/api/v1/auth/cli/poll"))
            .json(&json!({ "device_code": device_code }))
            .send()
            .await
            .context("poll login")?;
        if !poll.status().is_success() {
            continue;
        }
        let body: Value = poll.json().await.unwrap_or(Value::Null);
        if body["status"] == "ok" {
            return body["token"]
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| anyhow::anyhow!("no token"));
        }
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
        browser_login(&url).await?
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
    let dir = std::env::current_dir()?;
    // Existing git repo will push its own history; skip the server README commit
    // so the first `oh sync` is a fast-forward, not unrelated-histories.
    let linking = dir.join(".git").exists() && !repo_meta_path(&dir).exists();
    let res = client
        .post(format!("{}/api/v1/projects", creds.origin))
        .json(&json!({ "name": name, "init_readme": !linking }))
        .send()
        .await
        .context("create project")?;
    let status = res.status();
    let body: Value = res.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        bail!("create failed: {status} {body}");
    }
    let id = body["id"].as_str().unwrap_or("").to_string();
    println!("{id}");
    println!("slug={}", body["slug"].as_str().unwrap_or(""));
    if linking && !id.is_empty() {
        write_repo_meta(
            &dir,
            &RepoMeta {
                project_id: id,
                origin: creds.origin,
                github: None,
            },
        )?;
        println!("linked {}", dir.display());
        println!("next: oh sync");
    }
    Ok(())
}

async fn init_project(args: Vec<String>) -> Result<()> {
    if args.is_empty() {
        bail!("oh init <project-id>");
    }
    let identifier = &args[0];
    let dir = std::env::current_dir()?;
    if repo_meta_path(&dir).exists() {
        let meta = load_repo_meta(&dir)?;
        println!("already linked to {} ({})", meta.project_id, meta.origin);
        return Ok(());
    }
    let creds = load_creds()?;
    let client = client(&creds);
    let (project_id, _) = resolve_project(&client, &creds, identifier).await?;
    write_repo_meta(
        &dir,
        &RepoMeta {
            project_id: project_id.clone(),
            origin: creds.origin.clone(),
            github: None,
        },
    )?;
    println!("linked {} -> {project_id}", dir.display());
    println!("next: oh sync");
    Ok(())
}

fn github_cmd(args: Vec<String>) -> Result<()> {
    let dir = std::env::current_dir()?;
    let mut meta = load_repo_meta(&dir)?;
    let sub = args.first().map(String::as_str).unwrap_or("status");
    match sub {
        "status" | "" => {
            match meta.github.as_deref() {
                Some(url) => println!("github {url}"),
                None => println!("github not set (oh github set owner/repo)"),
            }
            Ok(())
        }
        "unset" | "clear" => {
            meta.github = None;
            write_repo_meta(&dir, &meta)?;
            println!("github unset");
            Ok(())
        }
        "set" => {
            let raw = args.get(1).cloned().or_else(|| origin_if_github(&dir));
            let Some(raw) = raw else {
                bail!("oh github set <owner/repo|url>  (or set git remote origin to github.com)");
            };
            let url = parse_github_url(&raw)?;
            meta.github = Some(url.clone());
            write_repo_meta(&dir, &meta)?;
            println!("github {url}");
            println!("next: oh sync  (pushes OpenHub, then GitHub)");
            Ok(())
        }
        other if !other.starts_with('-') && args.len() == 1 => {
            let url = parse_github_url(other)?;
            meta.github = Some(url.clone());
            write_repo_meta(&dir, &meta)?;
            println!("github {url}");
            println!("next: oh sync  (pushes OpenHub, then GitHub)");
            Ok(())
        }
        _ => bail!("oh github set [owner/repo|url] | oh github unset | oh github status"),
    }
}

fn parse_github_url(raw: &str) -> Result<String> {
    let s = raw.trim();
    if s.is_empty() {
        bail!("empty GitHub url");
    }
    if s.starts_with("git@github.com:") {
        let mut url = s.to_string();
        if !url.ends_with(".git") {
            url.push_str(".git");
        }
        return Ok(url);
    }
    if let Some(rest) = s
        .strip_prefix("https://github.com/")
        .or_else(|| s.strip_prefix("http://github.com/"))
        .or_else(|| s.strip_prefix("ssh://git@github.com/"))
    {
        let rest = rest.trim_matches('/');
        let rest = rest.trim_end_matches(".git");
        if rest.split('/').count() >= 2 {
            return Ok(format!("https://github.com/{rest}.git"));
        }
    }
    if let Some((owner, repo)) = s.split_once('/') {
        let repo = repo.trim_end_matches(".git");
        if !owner.is_empty() && !repo.is_empty() && !owner.contains(':') && !repo.contains('/') {
            return Ok(format!("https://github.com/{owner}/{repo}.git"));
        }
    }
    bail!("expected owner/repo or a github.com URL, got {raw}");
}

fn origin_if_github(dir: &Path) -> Option<String> {
    let url = git_stdout(dir, &["remote", "get-url", "origin"])?;
    if url.contains("github.com") {
        parse_github_url(&url).ok()
    } else {
        None
    }
}

/// Canonical project id plus the identifier to use in `/git/{…}` URLs.
async fn resolve_project(
    client: &reqwest::Client,
    creds: &Creds,
    identifier: &str,
) -> Result<(String, String)> {
    let mut identifier = identifier.to_string();
    if !identifier.contains('/') {
        let me_res = client
            .get(format!("{}/api/v1/me", creds.origin))
            .send()
            .await;
        if let Ok(res) = me_res {
            if res.status().is_success() {
                let body: Value = res.json().await.unwrap_or(Value::Null);
                if let Some(username) = body["username"].as_str() {
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
    Ok((project_id, identifier))
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
    let identifier_arg = positional[0].clone();
    let dir_arg = positional.get(1).cloned();
    let creds = load_creds()?;
    let client = client(&creds);
    let (project_id, identifier) = resolve_project(&client, &creds, &identifier_arg).await?;

    let dir = PathBuf::from(dir_arg.unwrap_or_else(|| {
        identifier
            .split('/')
            .next_back()
            .unwrap_or(&identifier)
            .to_string()
    }));

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
                github: None,
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
            github: None,
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
        let has_changes = git_with_openhub_ignore(&dir)
            .args(["status", "--porcelain", "--", ".", ":!.openhub"])
            .output()
            .ok()
            .map(|o| !o.stdout.is_empty())
            .unwrap_or(false);

        if has_changes {
            // Never commit OpenHub local cache (config + session events).
            // `.openhubignore` is extra gitignore syntax for this snapshot add.
            stage_sync_snapshot(&dir);

            let staged = Command::new("git")
                .args(["diff", "--cached", "--quiet"])
                .current_dir(&dir)
                .status()
                .ok()
                .map(|s| !s.success())
                .unwrap_or(false);

            if staged {
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
        }

        let git_url = format!("{}/git/{}", creds.origin, meta.project_id);
        let auth_header = format!("http.extraHeader=Authorization: Bearer {}", creds.token);

        let mut pushed = git_push_main(&dir, &auth_header, &git_url, false);
        if !pushed && remote_is_openhub_placeholder(&dir, &auth_header, &git_url) {
            println!("remote only has OpenHub placeholder commit; replacing with local history");
            pushed = git_push_main(&dir, &auth_header, &git_url, true);
        }
        if !pushed {
            bail!(
                "git push failed. Remote has commits not in this repo; pull/merge first (oh merge), then oh sync."
            );
        }

        let fetched = git_ok(&dir, &["-c", &auth_header, "fetch", &git_url]);
        if fetched {
            prefetch_critical_files(&dir);
        }

        if let Some(gh) = meta.github.as_deref() {
            println!("pushing to GitHub {gh}");
            if !git_ok(&dir, &["push", gh, "HEAD:refs/heads/main"]) {
                bail!("git push to GitHub failed (gh auth login, or SSH key for git@github.com)");
            }
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

    let mut known_ids: HashSet<String> = all_events
        .iter()
        .filter_map(|e| e.get("id").and_then(|id| id.as_str()).map(str::to_string))
        .collect();

    // 2. Discover .pi sessions; skip ids already in local cache (already pushed).
    let pi_events: Vec<Value> = collect_pi_sessions(dir)
        .into_iter()
        .filter(|e| {
            e.get("id")
                .and_then(|id| id.as_str())
                .map(|id| known_ids.insert(id.to_string()))
                .unwrap_or(true)
        })
        .collect();
    if pi_events.is_empty() {
        return Ok(());
    }
    println!("found {} new .pi session event(s) to sync", pi_events.len());

    let mut total_accepted = 0;
    let mut total_skipped = 0;

    // Bound each request to avoid HTTP 413 from reverse proxies (nginx
    // client_max_body_size defaults to 1 MB): max 100 events AND max 512 KB
    // of serialized payload. An event larger than the cap still ships alone.
    const CHUNK_MAX_EVENTS: usize = 100;
    const CHUNK_MAX_BYTES: usize = 512 * 1024;
    let mut chunk: Vec<&Value> = Vec::with_capacity(CHUNK_MAX_EVENTS);
    let mut chunk_bytes = 0usize;
    let mut chunks: Vec<Vec<&Value>> = Vec::new();
    for event in &pi_events {
        let event_bytes = serde_json::to_vec(event)
            .map(|v| v.len())
            .unwrap_or(CHUNK_MAX_BYTES);
        if !chunk.is_empty()
            && (chunk.len() >= CHUNK_MAX_EVENTS || chunk_bytes + event_bytes > CHUNK_MAX_BYTES)
        {
            chunks.push(std::mem::take(&mut chunk));
            chunk_bytes = 0;
        }
        chunk_bytes += event_bytes;
        chunk.push(event);
    }
    if !chunk.is_empty() {
        chunks.push(chunk);
    }

    for chunk in chunks {
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

fn openhub_ignore_file(dir: &Path) -> Option<PathBuf> {
    let path = dir.join(".openhubignore");
    path.is_file().then_some(path)
}

fn git_with_openhub_ignore(dir: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(dir);
    if let Some(ignore) = openhub_ignore_file(dir) {
        cmd.arg("-c")
            .arg(format!("core.excludesFile={}", ignore.display()));
    }
    cmd
}

/// Stage worktree for `oh sync`, honoring `.gitignore`, `.openhubignore`, and never `.openhub/`.
fn stage_sync_snapshot(dir: &Path) {
    let _ = git_with_openhub_ignore(dir)
        .args(["add", "-A", "--", ".", ":!.openhub"])
        .status();
}

fn git_ok(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .ok()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn git_stdout(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn git_push_main(dir: &Path, auth_header: &str, git_url: &str, force: bool) -> bool {
    let mut cmd = Command::new("git");
    cmd.args(["-c", auth_header, "push"]);
    if force {
        cmd.arg("--force");
    }
    cmd.args([git_url, "HEAD:refs/heads/main"])
        .current_dir(dir)
        .status()
        .ok()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// True when the remote is the single "Initial commit" README created at project create.
fn remote_is_openhub_placeholder(dir: &Path, auth_header: &str, git_url: &str) -> bool {
    if !git_ok(dir, &["-c", auth_header, "fetch", git_url]) {
        return false;
    }
    if git_stdout(dir, &["merge-base", "HEAD", "FETCH_HEAD"]).is_some() {
        return false;
    }
    let count = git_stdout(dir, &["rev-list", "--count", "FETCH_HEAD"]).unwrap_or_default();
    if count != "1" {
        return false;
    }
    git_stdout(dir, &["log", "-1", "--format=%s", "FETCH_HEAD"]).as_deref()
        == Some("Initial commit")
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn init_repo() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        assert!(
            Command::new("git")
                .args(["init", "-b", "main"])
                .current_dir(dir)
                .status()
                .unwrap()
                .success()
        );
        for (k, v) in [("user.email", "test@example.com"), ("user.name", "Test")] {
            assert!(
                Command::new("git")
                    .args(["config", k, v])
                    .current_dir(dir)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        tmp
    }

    fn staged_names(dir: &Path) -> HashSet<String> {
        git_stdout(dir, &["diff", "--cached", "--name-only"])
            .unwrap_or_default()
            .lines()
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn openhubignore_skips_files_and_folders_on_snapshot_add() {
        let tmp = init_repo();
        let dir = tmp.path();
        fs::write(
            dir.join(".openhubignore"),
            "# comments and globs\nsecret.txt\nbuild/\n*.log\n",
        )
        .unwrap();
        fs::write(dir.join("keep.txt"), "ok").unwrap();
        fs::write(dir.join("secret.txt"), "nope").unwrap();
        fs::write(dir.join("noise.log"), "log").unwrap();
        fs::create_dir_all(dir.join("build")).unwrap();
        fs::write(dir.join("build").join("out.bin"), "x").unwrap();
        fs::create_dir_all(dir.join(".openhub")).unwrap();
        fs::write(dir.join(".openhub").join("config.json"), "{}").unwrap();

        stage_sync_snapshot(dir);
        let staged = staged_names(dir);
        assert!(staged.contains("keep.txt"), "{staged:?}");
        assert!(staged.contains(".openhubignore"), "{staged:?}");
        assert!(!staged.contains("secret.txt"), "{staged:?}");
        assert!(!staged.contains("noise.log"), "{staged:?}");
        assert!(!staged.iter().any(|p| p.starts_with("build")), "{staged:?}");
        assert!(
            !staged
                .iter()
                .any(|p| p == ".openhub" || p.starts_with(".openhub/")),
            "{staged:?}"
        );
    }
}
