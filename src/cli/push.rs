use eyre::{Result, WrapErr, eyre};
use tokio::io::{AsyncReadExt, BufReader};

use crate::cli::{PushArgs, build_client_config, handle_response};
use crate::client::transport::{make_request, send_request};
use crate::client_actions::ClipboardBuildError;
use crate::protocol::{ClipboardValue, DEFAULT_MAX_SIZE, RequestKind};

pub async fn run(args: PushArgs) -> Result<()> {
    let effective_max_size = if args.max_size == 0 {
        DEFAULT_MAX_SIZE
    } else {
        args.max_size
    };

    let value = match build_clipboard_value(args.stdin, effective_max_size).await {
        Ok(value) => value,
        Err(err) => return crate::cli::exit::exit_with_code(err.code, &err.message),
    };

    let response = match send_request(
        &build_client_config(
            args.target,
            args.host,
            args.user,
            args.port,
            args.identity_file,
            args.ssh_option,
            args.ssh_bin,
            effective_max_size,
            args.timeout_ms,
            args.strict_frames,
            args.resync_max_bytes,
        ),
        make_request(RequestKind::Set { value }),
    )
    .await
    {
        Ok(response) => response,
        Err(err) => return crate::cli::exit::exit_with_code(5, &err.to_string()),
    };

    handle_response(response, false)
}

async fn build_clipboard_value(
    stdin: bool,
    max_size: usize,
) -> Result<ClipboardValue, ClipboardBuildError> {
    if stdin {
        let text = read_stdin_text().await.map_err(|err| ClipboardBuildError {
            code: 2,
            message: err.to_string(),
        })?;
        return crate::client_actions::build_text_value(text, max_size);
    }

    crate::client_actions::build_clipboard_value_from_clipboard(max_size)
}

async fn read_stdin_text() -> Result<String> {
    let mut reader = BufReader::new(tokio::io::stdin());
    let mut buffer = String::new();
    reader
        .read_to_string(&mut buffer)
        .await
        .wrap_err("failed to read stdin")?;
    if buffer.is_empty() {
        return Err(eyre!("stdin was empty"));
    }
    Ok(buffer)
}
