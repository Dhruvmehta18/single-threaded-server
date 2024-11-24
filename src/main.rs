use std::net::SocketAddr;
use env_logger::Env;
use http_body_util::{Empty, Full};
use http_body_util::{BodyExt, combinators::BoxBody};
use hyper::{Request, Response};
use hyper::{Method, StatusCode};
use hyper::body::{Bytes};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use log::{info, warn};

// We create some utility functions to make Empty and Full bodies
// fit our broadened Response body type.
fn empty() -> BoxBody<Bytes, hyper::Error> {
    Empty::<Bytes>::new()
        .map_err(|never| match never {})
        .boxed()
}
fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, hyper::Error> {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}

async fn echo(
    req: Request<hyper::body::Incoming>,
) -> Result<Response<BoxBody<Bytes, hyper::Error>>, hyper::Error> {
    info!("Received request: {:?}", req);
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/echo")=>Ok(Response::new(full(
            "Hello World!"
        ))),
        _ => {
            let mut not_found = Response::new(empty());
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(not_found)
        }
    }
}

pub(crate) async fn handle_connection() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // We create a TcpListener and bind it to 127.0.0.1:$PORT
    let port = get_env_variable("PORT", 7777);
    let addr = SocketAddr::from(([127,0,0,1], port as u16));
    let listener = TcpListener::bind(addr).await?;
    info!("Listening on http://{}", addr);

    // We start a loop to continuously accept incoming connections
    loop {
        let (stream, _) = listener.accept().await?;

        let io = TokioIo::new(stream);

        // Spawn a tokio task to serve multiple connections concurrently
        tokio::task::spawn(async move {
            let bound_thread = std::thread::current();
            let thread_name = bound_thread.name().unwrap_or("unknown");
            let thread_id = bound_thread.id();
            log::info!("Spawning task on thread: {} with id: {:?}", thread_name, thread_id);
            // Finally, we bind the incoming connection to our `hello` service
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(move |request| echo(request)))
                .await
            {
                eprintln!("Error serving connection: {:?}", err);
            }
        });
    }
}

pub fn get_env_variable(key: &str, default: i32) -> i32 {
    match std::env::var(key) {
        core::result::Result::Ok(val) => val.parse::<i32>().unwrap(),
        core::result::Result::Err(_) => default,
    }
}


#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let base_port: i32 = get_env_variable("PORT_BASE", 7777);
    let node_app_instance = get_env_variable("NODE_APP_INSTANCE", 0);
    let port = base_port + node_app_instance + 1;
    println!("PORT_BASE: {}", base_port);
    println!("NODE_APP_INSTANCE: {}", get_env_variable("NODE_APP_INSTANCE", 0));
    println!("PORT: {}", port);
    std::env::set_var("PORT", port.to_string());
    return handle_connection().await;
}
