use std::{fs, io};
use std::io::Write;
use std::net::SocketAddr;
use std::path::Path;
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
use log::{info};
use regex::Regex;
use num_cpus;

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

pub(crate) async fn handle_connection(port: usize) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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


fn make_changes_in_nginx_file(base_port: usize) -> io::Result<()> {
    // Determine the number of CPU cores using num_cpus (cross-platform)
    let cores = num_cpus::get();
    println!("Detected {} CPU cores.", cores);

    // Define the starting port
    let start_port = base_port+1;
    let mut servers = String::new();

    // Create server entries with incrementing ports
    for i in 0..cores {
        let port = start_port + i;
        servers.push_str(&format!("        server 127.0.0.1:{};\n", port));
    }
    // Read the NGINX configuration file which is located at the root of the project
    let conf_path = Path::new("nginx.conf");

    if !conf_path.exists() {
        println!("nginx.conf file not found at: {}", conf_path.display());
        return Err(io::Error::new(io::ErrorKind::NotFound, "nginx.conf file not found"));
    }

    let backup_path = Path::new("nginx.conf.bak");
    let content = fs::read_to_string(conf_path)?;

    // Create a backup of the original file
    if !fs::metadata(&backup_path).is_ok() {
        fs::copy(conf_path, &backup_path)?;
        println!("Backup created at: {}", backup_path.display());
    }

    // Regex to match the content inside the `upstream backend_servers { ... }` block
    let re = Regex::new(r"(?s)upstream backend_servers \{.*?}").unwrap();

    // Create the new block content
    let new_block = format!("upstream backend_servers {{\n{}    }}", servers);

    // Replace the matched block with the new content
    let new_content = re.replace(&content, new_block.as_str());

    // Write the updated configuration back to the file
    let mut file = fs::File::create(conf_path)?;
    file.write_all(new_content.as_bytes())?;

    println!("Updated the 'upstream backend_servers' block successfully!");

    Ok(())
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
    if node_app_instance == 0 {
        make_changes_in_nginx_file(base_port as usize).unwrap();
        return handle_connection(base_port as usize).await;
    } else {
        return handle_connection(base_port as usize).await;
    }
}
