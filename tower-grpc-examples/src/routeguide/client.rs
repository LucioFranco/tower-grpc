#![allow(dead_code)]
#![allow(unused_variables)]

extern crate bytes;
extern crate env_logger;
extern crate futures;
extern crate http;
extern crate hyper;
extern crate log;
extern crate prost;
extern crate tokio;
extern crate tower_grpc;
extern crate tower_hyper;
extern crate tower_request_modifier;
extern crate tower_service;
extern crate tower_util;

extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

use futures::{Future, Stream};
use hyper::client::connect::{Destination, HttpConnector};
use std::time::{Duration, Instant};
use tokio::timer::Interval;
use tower_grpc::Request;
use tower_hyper::{client, util};
use tower_util::MakeService;

use routeguide::{Point, RouteNote};

mod data;
pub mod routeguide {
    include!(concat!(env!("OUT_DIR"), "/routeguide.rs"));
}

pub fn main() {
    let _ = ::env_logger::init();

    let uri: http::Uri = format!("http://localhost:10000").parse().unwrap();

    let dst = Destination::try_from_uri(uri.clone()).unwrap();
    let connector = util::Connector::new(HttpConnector::new(4));
    let mut make_client = client::Connect::new(connector, client::Builder::new());

    let rg = make_client
        .make_service(dst)
        .map_err(|e| {
            panic!("HTTP/2 connection failed; err={:?}", e);
        })
        .map(move |conn| {
            use routeguide::client::RouteGuide;

            let conn = tower_request_modifier::Builder::new()
                .set_origin(uri)
                .build(conn)
                .unwrap();

            RouteGuide::new(conn)
        })
        .and_then(|mut client| {
            let start = Instant::now();
            let r_feature = client
                .get_feature(Request::new(Point {
                    latitude: 409146138,
                    longitude: -746188906,
                }))
                .map_err(|e| eprintln!("GetFeature request failed; err={:?}", e))
                .map(|response| {
                    println!("RESPONSE = {:?}", response);
                });
            let outbound = Interval::new_interval(Duration::from_secs(1))
                .map(move |t| {
                    let elapsed = t.duration_since(start);
                    RouteNote {
                        location: Some(Point {
                            latitude: 409146138 + elapsed.as_secs() as i32,
                            longitude: -746188906,
                        }),
                        message: format!("at {:?}", elapsed),
                    }
                })
                .map_err(|e| panic!("timer error; err={:?}", e));
            let r_chat = client
                .route_chat(Request::new(outbound))
                .map_err(|e| {
                    eprintln!("RouteChat request failed; err={:?}", e);
                })
                .and_then(|response| {
                    let inbound = response.into_inner();
                    inbound
                        .for_each(|note| {
                            println!("NOTE = {:?}", note);
                            Ok(())
                        })
                        .map_err(|e| eprintln!("gRPC inbound stream error: {:?}", e))
                });
            tokio::spawn(r_feature.and_then(|()| r_chat))
        });

    tokio::run(rg);
}
