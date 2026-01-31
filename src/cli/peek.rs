use eyre::Result;

use crate::cli::{PeekArgs, build_client_config, handle_peek_response};
use crate::client::transport::{make_request, send_request};
use crate::protocol::RequestKind;

pub async fn run(args: PeekArgs) -> Result<()> {
    let response = match send_request(
        &build_client_config(
            args.target,
            args.host,
            args.user,
            args.port,
            args.identity_file,
            args.ssh_option,
            args.ssh_bin,
            args.max_size,
            args.timeout_ms,
            args.strict_frames,
            args.resync_max_bytes,
        ),
        make_request(RequestKind::PeekMeta),
    )
    .await
    {
        Ok(response) => response,
        Err(err) => return crate::cli::exit::exit_with_code(5, &err.to_string()),
    };
    handle_peek_response(response, args.json)
}
