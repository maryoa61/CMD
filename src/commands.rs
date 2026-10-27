use crate::shell::Shell;
use crate::vfs::VfsError;

pub async fn ls(shell: &Shell, args: &[&str]) -> Result<String, VfsError> {
    let path = if args.is_empty() { &shell.cwd } else { args[0] };
    let abs = resolve_path(&shell.cwd, path);
    let names = shell.vfs.ls(&abs)?;
    Ok(names.join("  ") + "\n")
}

pub async fn cd(shell: &mut Shell, args: &[&str]) -> Result<String, VfsError> {
    let target = if args.is_empty() {
        shell.env.get("HOME").cloned().unwrap_or_else(|| "/root".into())
    } else {
        resolve_path(&shell.cwd, args[0])
    };
    // verify it is a directory
    shell.vfs.ls(&target)?;
    shell.cwd = target;
    Ok(String::new())
}

pub async fn cat(shell: &Shell, args: &[&str]) -> Result<String, VfsError> {
    if args.is_empty() {
        return Ok("cat: missing operand\n".into());
    }
    let mut out = String::new();
    for a in args {
        let abs = resolve_path(&shell.cwd, a);
        let data = shell.vfs.read_file(&abs).await?;
        out.push_str(&String::from_utf8_lossy(&data));
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

pub async fn mkdir(shell: &mut Shell, args: &[&str]) -> Result<String, VfsError> {
    for a in args {
        let abs = resolve_path(&shell.cwd, a);
        shell.vfs.mkdir(&abs).await?;
    }
    Ok(String::new())
}

pub async fn rm(shell: &mut Shell, args: &[&str]) -> Result<String, VfsError> {
    let mut recursive = false;
    let mut paths = Vec::new();
    for a in args {
        if *a == "-r" || *a == "-rf" {
            recursive = true;
        } else {
            paths.push(*a);
        }
    }
    for p in paths {
        let abs = resolve_path(&shell.cwd, p);
        shell.vfs.rm(&abs, recursive).await?;
    }
    Ok(String::new())
}

pub async fn cp(shell: &mut Shell, args: &[&str]) -> Result<String, VfsError> {
    if args.len() < 2 {
        return Ok("cp: missing operand\n".into());
    }
    let src = resolve_path(&shell.cwd, args[0]);
    let dst = resolve_path(&shell.cwd, args[1]);
    shell.vfs.cp(&src, &dst).await?;
    Ok(String::new())
}

pub async fn mv(shell: &mut Shell, args: &[&str]) -> Result<String, VfsError> {
    if args.len() < 2 {
        return Ok("mv: missing operand\n".into());
    }
    let src = resolve_path(&shell.cwd, args[0]);
    let dst = resolve_path(&shell.cwd, args[1]);
    shell.vfs.mv(&src, &dst).await?;
    Ok(String::new())
}

pub async fn grep(shell: &Shell, args: &[&str]) -> Result<String, VfsError> {
    if args.len() < 2 {
        return Ok("grep: usage: grep PATTERN FILE\n".into());
    }
    let pattern = args[0];
    let file = resolve_path(&shell.cwd, args[1]);
    let data = shell.vfs.read_file(&file).await?;
    let text = String::from_utf8_lossy(&data);
    let mut out = String::new();
    for line in text.lines() {
        if line.contains(pattern) {
            out.push_str(line);
            out.push('\n');
        }
    }
    Ok(out)
}

fn resolve_path(cwd: &str, path: &str) -> String {
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("{}/{}", cwd.trim_end_matches('/'), path)
    }
}
