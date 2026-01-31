use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use eyre::Result;
use std::fs;

use crate::cli::{ClientConfigArgs, PullArgs, build_client_config, handle_peek_response};
use crate::client::transport::{make_request, send_request};
use crate::client_actions::{PullApplyErrorKind, apply_pull_response_with_system_clipboard};
use crate::protocol::{CONTENT_TYPE_PNG, CONTENT_TYPE_TEXT, RequestKind, ResponseKind};

pub async fn run(args: PullArgs) -> Result<()> {
    if args.stdout && args.output.is_some() {
        return crate::cli::exit::exit_with_code(2, "use either --stdout or --output, not both");
    }
    if args.base64 && !args.stdout {
        return crate::cli::exit::exit_with_code(2, "--base64 requires --stdout");
    }

    let effective_max_size = if args.max_size == 0 {
        crate::protocol::DEFAULT_MAX_SIZE
    } else {
        args.max_size
    };

    if args.peek {
        let response = match send_request(
            &build_client_config(client_config_args(&args, effective_max_size)),
            make_request(RequestKind::PeekMeta),
        )
        .await
        {
            Ok(response) => response,
            Err(err) => return crate::cli::exit::exit_with_code(5, &err.to_string()),
        };
        return handle_peek_response(response, args.json);
    }

    let response = match send_request(
        &build_client_config(client_config_args(&args, effective_max_size)),
        make_request(RequestKind::Get),
    )
    .await
    {
        Ok(response) => response,
        Err(err) => return crate::cli::exit::exit_with_code(5, &err.to_string()),
    };

    if !args.stdout && args.output.is_none() && !args.base64 {
        return handle_pull_to_clipboard(response, effective_max_size);
    }

    if let ResponseKind::Value { value } = &response.kind {
        if value.content_type == CONTENT_TYPE_TEXT {
            let text = match String::from_utf8(value.data.clone()) {
                Ok(text) => text,
                Err(_) => {
                    return crate::cli::exit::exit_with_code(2, "response was not valid UTF-8");
                }
            };
            if args.stdout {
                println!("{text}");
                return Ok(());
            }
            if let Some(path) = args.output {
                if let Err(err) = fs::write(&path, text.as_bytes()) {
                    return crate::cli::exit::exit_with_code(
                        2,
                        &format!("failed to write output: {err}"),
                    );
                }
                return Ok(());
            }
        }

        if value.content_type == CONTENT_TYPE_PNG {
            if let Some(path) = args.output {
                if let Err(err) = fs::write(&path, &value.data) {
                    return crate::cli::exit::exit_with_code(
                        2,
                        &format!("failed to write output: {err}"),
                    );
                }
                return Ok(());
            }
            if args.base64 && args.stdout {
                let encoded = STANDARD.encode(&value.data);
                println!("{encoded}");
                return Ok(());
            }
            if args.stdout {
                return crate::cli::exit::exit_with_code(
                    2,
                    "use --base64 or --output for image data",
                );
            }
        }

        if let Some(path) = args.output {
            if let Err(err) = fs::write(&path, &value.data) {
                return crate::cli::exit::exit_with_code(
                    2,
                    &format!("failed to write output: {err}"),
                );
            }
            return Ok(());
        }
        if args.base64 && args.stdout {
            let encoded = STANDARD.encode(&value.data);
            println!("{encoded}");
            return Ok(());
        }
        return crate::cli::exit::exit_with_code(
            2,
            &format!("unsupported content type: {}", value.content_type),
        );
    }

    crate::cli::handle_response(response, false)
}

fn handle_pull_to_clipboard(
    response: crate::protocol::Response,
    max_decoded_bytes: usize,
) -> Result<()> {
    match apply_pull_response_with_system_clipboard(response, max_decoded_bytes) {
        Ok(()) => Ok(()),
        Err(err) => match err.kind {
            PullApplyErrorKind::Clipboard => crate::cli::exit::exit_with_code(6, &err.message),
            PullApplyErrorKind::NoValue => crate::cli::exit::exit_with_code(2, &err.message),
            PullApplyErrorKind::InvalidUtf8
            | PullApplyErrorKind::InvalidPayload
            | PullApplyErrorKind::UnsupportedContentType
            | PullApplyErrorKind::Server
            | PullApplyErrorKind::Unexpected => crate::cli::exit::exit_with_code(2, &err.message),
        },
    }
}

fn client_config_args(args: &PullArgs, max_size: usize) -> ClientConfigArgs {
    ClientConfigArgs {
        target: args.target.clone(),
        host: args.host.clone(),
        user: args.user.clone(),
        port: args.port,
        identity_file: args.identity_file.clone(),
        ssh_option: args.ssh_option.clone(),
        ssh_bin: args.ssh_bin.clone(),
        max_size,
        timeout_ms: args.timeout_ms,
        strict_frames: args.strict_frames,
        resync_max_bytes: args.resync_max_bytes,
    }
}
