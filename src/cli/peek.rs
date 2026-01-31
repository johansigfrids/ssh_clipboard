use eyre::Result;

use crate::cli::{ClientConfigArgs, PeekArgs, build_client_config, handle_peek_response};
use crate::client::transport::{make_request, send_request};
use crate::protocol::RequestKind;

pub async fn run(args: PeekArgs) -> Result<()> {
    let response = match send_request(
        &build_client_config(ClientConfigArgs {
            target: args.target,
            host: args.host,
            user: args.user,
            port: args.port,
            identity_file: args.identity_file,
            ssh_option: args.ssh_option,
            ssh_bin: args.ssh_bin,
            max_size: args.max_size,
            timeout_ms: args.timeout_ms,
            strict_frames: args.strict_frames,
            resync_max_bytes: args.resync_max_bytes,
        }),
        make_request(RequestKind::PeekMeta),
    )
    .await
    {
        Ok(response) => response,
        Err(err) => return crate::cli::exit::exit_with_code(5, &err.to_string()),
    };
    handle_peek_response(response, args.json)
}
