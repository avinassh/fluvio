use std::sync::Arc;

use async_trait::async_trait;
use tracing::{debug, warn};

use fluvio_service::{wait_for_request, FlvService};
use fluvio_socket::{FlvSocket, FlvSocketError};
use fluvio_future::net::TcpStream;

use crate::core::DefaultSharedGlobalContext;
use crate::replication::leader::FollowerHandler;
use super::SpuPeerRequest;
use super::SPUPeerApiEnum;
use super::FetchStreamResponse;

#[derive(Debug)]
pub struct InternalService {}

impl InternalService {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl FlvService<TcpStream> for InternalService {
    type Context = DefaultSharedGlobalContext;
    type Request = SpuPeerRequest;

    async fn respond(
        self: Arc<Self>,
        ctx: DefaultSharedGlobalContext,
        socket: FlvSocket,
    ) -> Result<(), FlvSocketError> {
        let (mut sink, mut stream) = socket.split();
        let mut api_stream = stream.api_stream::<SpuPeerRequest, SPUPeerApiEnum>();

        // register follower
        let (follower_id, spu_update) = wait_for_request!(
            api_stream,

            SpuPeerRequest::FetchStream(req_msg) => {

                let request = &req_msg.request;
                let follower_id = request.spu_id;
                debug!(
                    follower_id,
                    "received fetch stream"
                );
                // check if follower_id is valid
                if let Some(spu_update) = ctx.follower_updates().get(&follower_id).await {
                    let response = FetchStreamResponse::new(follower_id);
                    let res_msg = req_msg.new_response(response);
                    sink
                        .send_response(&res_msg, req_msg.header.api_version())
                        .await?;
                    (follower_id,spu_update)
                } else {
                    warn!(follower_id, "unknown spu, dropping connection");
                    return Ok(())
                }

            }
        );

        drop(api_stream);

        FollowerHandler::start(ctx, follower_id, spu_update, sink, stream).await;

        debug!("finishing SPU peer loop");
        Ok(())
    }
}
