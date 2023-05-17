// Copyright 2020-2023 The NATS Authors
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    task::Poll,
    time::Instant,
};

use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use tracing::{debug, trace};

use crate::{Client, Subscriber};

use super::{error, Endpoints, Request, ShutdownReceiverFuture};

pub struct Endpoint {
    pub(crate) requests: Subscriber,
    pub(crate) stats: Arc<Mutex<Endpoints>>,
    pub(crate) client: Client,
    pub(crate) endpoint: String,
    pub(crate) shutdown: Option<tokio::sync::broadcast::Receiver<()>>,
    pub(crate) shutdown_future: Option<ShutdownReceiverFuture>,
}

impl Stream for Endpoint {
    type Item = Request;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        trace!("polling for next request");
        match self.shutdown_future.as_mut() {
            Some(shutdown) => match shutdown.as_mut().poll(cx) {
                Poll::Ready(_result) => {
                    debug!("got stop broadcast");
                    self.requests
                        .sender
                        .try_send(crate::Command::Unsubscribe {
                            sid: self.requests.sid,
                            max: None,
                        })
                        .ok();
                }
                Poll::Pending => {
                    trace!("stop broadcast still pending");
                }
            },
            None => {
                let mut receiver = self.shutdown.take().unwrap();
                self.shutdown_future = Some(Box::pin(async move { receiver.recv().await }));
            }
        }
        trace!("checking for new messages");
        match self.requests.poll_next_unpin(cx) {
            Poll::Ready(message) => {
                debug!("got next message");
                match message {
                    Some(message) => Poll::Ready(Some(Request {
                        issued: Instant::now(),
                        stats: self.stats.clone(),
                        client: self.client.clone(),
                        message,
                        endpoint: self.endpoint.clone(),
                    })),
                    None => Poll::Ready(None),
                }
            }

            Poll::Pending => {
                trace!("still pending for messages");
                Poll::Pending
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

impl Endpoint {
    /// Stops the [Endpoint] and unsubscribes from the subject.
    pub async fn stop(&mut self) -> Result<(), std::io::Error> {
        self.requests
            .unsubscribe()
            .await
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::Other, "failed to unsubscribe"))
    }
}

/// Stats of a single endpoint.
/// Right now, there is only one business endpoint, all other are internals.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub(crate) struct Inner {
    // Response type.
    #[serde(rename = "type")]
    pub(crate) kind: String,
    /// Endpoint name.
    pub(crate) name: String,
    /// The subject on which the endpoint is registered
    pub(crate) subject: String,
    /// Endpoint specific metadata
    pub(crate) metadata: HashMap<String, String>,
    /// Number of requests handled.
    #[serde(rename = "num_requests")]
    pub(crate) requests: usize,
    /// Number of errors occurred.
    #[serde(rename = "num_errors")]
    pub(crate) errors: usize,
    /// Total processing time for all requests.
    #[serde(default, with = "serde_nanos")]
    pub(crate) processing_time: std::time::Duration,
    /// Average processing time for request.
    #[serde(default, with = "serde_nanos")]
    pub(crate) average_processing_time: std::time::Duration,
    /// Last error that occurred.
    pub(crate) last_error: Option<error::Error>,
    /// Custom data added by [Config::stats_handler]
    pub(crate) data: String,
    /// EndpointSchema
    pub(crate) schema: Option<Schema>,
}

impl From<Inner> for Stats {
    fn from(inner: Inner) -> Self {
        Stats {
            kind: inner.kind,
            name: inner.name,
            subject: inner.subject,
            metadata: inner.metadata,
            requests: inner.requests,
            errors: inner.errors,
            processing_time: inner.processing_time,
            average_processing_time: inner.average_processing_time,
            last_error: inner.last_error,
            data: inner.data,
        }
    }
}

/// Schema of requests and responses.
/// Currently, it does not do anything except providing information.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct Schema {
    /// A string/url describing the format of the request payload can be JSON schema etc.
    pub request: String,
    /// A string/url describing the format of the request payload can be JSON schema etc.
    pub response: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Stats {
    // Response type.
    #[serde(rename = "type")]
    pub kind: String,
    /// Endpoint name.
    pub name: String,
    /// The subject on which the endpoint is registered
    pub subject: String,
    /// Endpoint specific metadata
    pub metadata: HashMap<String, String>,
    /// Number of requests handled.
    #[serde(rename = "num_requests")]
    pub requests: usize,
    /// Number of errors occurred.
    #[serde(rename = "num_errors")]
    pub errors: usize,
    /// Total processing time for all requests.
    #[serde(default, with = "serde_nanos")]
    pub processing_time: std::time::Duration,
    /// Average processing time for request.
    #[serde(default, with = "serde_nanos")]
    pub average_processing_time: std::time::Duration,
    /// Last error that occurred.
    pub last_error: Option<error::Error>,
    /// Custom data added by [crate::service::Config::stats_handler]
    pub data: String,
}
