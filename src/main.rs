use kvarn::prelude::*;

const DATA_DIR: &str = "data";

#[tokio::main(flavor = "current_thread")]
async fn main() {
    env_logger::init();

    let _ = tokio::fs::DirBuilder::new().create(DATA_DIR).await;

    let mut extensions = Extensions::new();

    extensions.add_prepare_single("/list".to_owned(), prepare!(req, host, _path, _addr {
        let query = parse::query(req.uri().query().unwrap_or(""));
        let id = query.get("id");

        if let Some(id) = id {
            let contains_illegal_chars = id.chars().any(|char| !(char.is_ascii_alphanumeric() || char == '_' || char == '-'));

            if contains_illegal_chars {
                return utility::default_error_response(StatusCode::BAD_REQUEST, host, None).await;
            }

            match *req.method() {
                Method::GET => {
                    async fn read_file(path: &Path) -> io::Result<Bytes> {
                        let mut file = tokio::fs::File::open(path).await?;
                        let mut buffer = BytesMut::with_capacity(4096);
                        utility::read_to_end(&mut buffer, &mut file).await?;
                        Ok(buffer.freeze())
                    }
                    let mut path = PathBuf::new();
                    path.push(DATA_DIR);
                    path.push(id);

                    info!("Reading id {}", id);

                    let body = read_file(&path).await;

                    match body {
                        Ok(body) => {
                            let response = Response::new(body);
                            FatResponse::no_cache(response)
                        }
                        Err(_) => {
                            FatResponse::no_cache(Response::new(Bytes::from_static(b"{}")))
                        }
                    }
                }
                Method::PUT => {
                    async fn read_write_file(path: &Path, body: &mut application::Body) -> io::Result<()> {
                        let content = body.read_to_bytes().await?;

                        let mut file = tokio::fs::File::create(path).await?;
                        file.write_all(&content).await?;
                        Ok(())
                    }

                    let mut path = PathBuf::new();
                    path.push(DATA_DIR);
                    path.push(id);

                    info!("Writing id {}", id);

                    if read_write_file(&path, req.body_mut()).await.is_err() {
                        utility::default_error_response(StatusCode::INTERNAL_SERVER_ERROR, host, None).await
                    } else {
                        FatResponse::no_cache(Response::new(Bytes::new()))
                    }
                }
                _ => {
                    utility::default_error_response(StatusCode::METHOD_NOT_ALLOWED, host, None).await
                }
            }
        } else {
            utility::default_error_response(StatusCode::BAD_REQUEST, host, Some("You need an ID in the query.")).await
        }
    }));

    // Create a host with hostname "localhost", serving files from directory "./public", and the default extensions.
    let host = Host::non_secure(
        "localhost",
        PathBuf::from("./"),
        extensions,
        host::Options::new(),
    );
    // Create a set of virtual hosts (`Data`) with `host` as the default.
    let data = Data::builder(host).build();
    // Bind port 8080 with `data`.
    let port_descriptor = PortDescriptor::new(8080, data);

    // Run with the configured ports.
    let shutdown_manager = run(vec![port_descriptor]).await;
    // Waits for shutdown.
    shutdown_manager.wait().await;
}
