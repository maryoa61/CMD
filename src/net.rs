use crate::shell::Shell;
use crate::vfs::VfsError;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{window, Request, RequestInit, RequestMode, Response, Headers};
use js_sys::Uint8Array;

pub async fn curl(args: &[&str]) -> Result<String, VfsError> {
    if args.is_empty() {
        return Ok("curl: try 'curl --help'\n".into());
    }
    let url = args[0];
    let window = window().ok_or(VfsError::Io("no window".into()))?;
    let mut opts = RequestInit::new();
    opts.method("GET");
    opts.mode(RequestMode::Cors);

    let request = Request::new_with_str_and_init(url, &opts)
        .map_err(|e| VfsError::Io(format!("{:?}", e)))?;

    let resp_value = JsFuture::from(window.fetch_with_request(&request)).await
        .map_err(|e| VfsError::Io(format!("{:?}", e)))?;
    let resp: Response = resp_value.dyn_into()
        .map_err(|e| VfsError::Io(format!("{:?}", e)))?;

    let text = JsFuture::from(resp.text().map_err(|e| VfsError::Io(format!("{:?}", e)))?).await
        .map_err(|e| VfsError::Io(format!("{:?}", e)))?;
    Ok(text.as_string().unwrap_or_default() + "\n")
}

pub async fn wget(shell: &mut Shell, args: &[&str]) -> Result<String, VfsError> {
    if args.is_empty() {
        return Ok("wget: missing URL\n".into());
    }
    let url = args[0];
    let filename = url.rsplit('/').next().unwrap_or("index.html");

    let window = window().ok_or(VfsError::Io("no window".into()))?;
    let mut opts = RequestInit::new();
    opts.method("GET");
    opts.mode(RequestMode::Cors);

    let request = Request::new_with_str_and_init(url, &opts)
        .map_err(|e| VfsError::Io(format!("{:?}", e)))?;

    let resp_value = JsFuture::from(window.fetch_with_request(&request)).await
        .map_err(|e| VfsError::Io(format!("{:?}", e)))?;
    let resp: Response = resp_value.dyn_into()
        .map_err(|e| VfsError::Io(format!("{:?}", e)))?;

    let ab = JsFuture::from(resp.array_buffer().map_err(|e| VfsError::Io(format!("{:?}", e)))?).await
        .map_err(|e| VfsError::Io(format!("{:?}", e)))?;
    let arr = Uint8Array::new(&ab);
    let mut buf = vec![0u8; arr.length() as usize];
    arr.copy_to(&mut buf);

    let path = format!("{}/{}", shell.cwd, filename);
    shell.vfs.write_file(&path, &buf).await?;
    Ok(format!("saved '{}'\n", path))
}

pub async fn ping(args: &[&str]) -> Result<String, VfsError> {
    // Browser cannot do real ICMP; we approximate with a HEAD/GET timing
    if args.is_empty() {
        return Ok("ping: usage: ping HOST\n".into());
    }
    let host = args[0];
    let url = if host.starts_with("http") { host.to_string() } else { format!("https://{}", host) };

    let window = window().ok_or(VfsError::Io("no window".into()))?;
    let perf = window.performance().ok_or(VfsError::Io("no performance".into()))?;
    let start = perf.now();

    let mut opts = RequestInit::new();
    opts.method("HEAD");
    opts.mode(RequestMode::Cors);

    let request = Request::new_with_str_and_init(&url, &opts)
        .map_err(|e| VfsError::Io(format!("{:?}", e)))?;

    match JsFuture::from(window.fetch_with_request(&request)).await {
        Ok(_) => {
            let ms = perf.now() - start;
            Ok(format!("PING {} : time={:.1} ms (HTTP)\n", host, ms))
        }
        Err(e) => Ok(format!("ping: {}: {}\n", host, format!("{:?}", e))),
    }
}
