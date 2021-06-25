use kvarn::prelude::*;

// This below is a documentation comment.
// They exist in Rust and provide a way for you to add documentation to your types.
// Hover over the name `DATA_DIR` below to see the comment!
/// The data directory storing the lists.
const DATA_DIR: &str = "data";

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // Init the logger
    env_logger::init();

    // Create the folder `DATA_DIR`
    tokio::fs::DirBuilder::new().create(DATA_DIR).await.unwrap();

    // Create a new set of extensions
    let mut extensions = Extensions::new();

    // Handle requests to `/list` with this code.
    // Here, we define the API which handles saving and retrieving lists.
    extensions.add_prepare_single("/list".to_owned(), prepare!(req, host, _path, _addr {
        // Parse (a Kvarn function) the query, the part of the URL (or URI as it's officially called) after the `?`.
        let query = parse::query(req.uri().query().unwrap_or(""));
        // Get the `id` part of the query.
        let id = query.get("id");

        // If the id is present in the query, do this
        if let Some(id) = id {
            // See if any characters of the id are considered illegal by us.
            let contains_illegal_chars = id.chars().any(|char| !(char.is_ascii_alphanumeric() || char == '_' || char == '-'));

            if contains_illegal_chars {
                return utility::default_error_response(StatusCode::BAD_REQUEST, host, None).await;
            }

            // See which method was requested.
            // Return the list if the method is GET
            // and save it if the method is PUT.
            match *req.method() {
                Method::GET => {
                    // Define a function to read from a file and get the bytes.
                    // This is here so we can use `?` to return if a error occurs.
                    // We then only have to handle an error once, when we call this function below.
                    async fn read_file(path: &Path) -> io::Result<Bytes> {
                        let mut file = tokio::fs::File::open(path).await?;
                        let mut buffer = BytesMut::with_capacity(4096);
                        utility::read_to_end(&mut buffer, &mut file).await?;
                        Ok(buffer.freeze())
                    }
                    // Create a new path
                    let mut path = PathBuf::new();
                    // Add DATA_DIR to it
                    path.push(DATA_DIR);
                    // Then add the id, as the filename
                    path.push(id);

                    // This is a log.
                    info!("Reading id {}", id);

                    // Read the file
                    let body = read_file(&path).await;
                    
                    // If the operation was successful, return
                    // (the last thing with no `;` at the end)
                    // a new response,
                    // else a empty response.
                    //
                    // The from_static takes a byte input (which can be written as b"bytes" in Rust)
                    // and creates a Bytes object from it.
                    // The `{}` signals to the JS requesting this that the list is empty.
                    // If we didn't include the `{}`, the `JSON.parse` would fail, as it wouldn't
                    // be valid JSON, which our JS expects.
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
                    // Same here, we define a function to simplify error handling.
                    // The body is a Kvarn type of a stream we can get the bytes from.
                    async fn read_write_file(path: &Path, body: &mut application::Body) -> io::Result<()> {
                        // Get the bytes from the client (the JS)
                        let content = body.read_to_bytes().await?;

                        // Don't accept lists larger than 128KB (1024 * 128 bytes) in size
                        if content.len() >= 1024 * 128 {
                            return Err(io::Error::new(io::ErrorKind::InvalidData, "data too long"));
                        }

                        // Create the file
                        let mut file = tokio::fs::File::create(path).await?;
                        // Write all the data
                        file.write_all(&content).await?;
                        Ok(())
                    }

                    let mut path = PathBuf::new();
                    path.push(DATA_DIR);
                    path.push(id);

                    info!("Writing id {}", id);

                    // If the read_write_file function returns a error, run this
                    if let Err(err) = read_write_file(&path, req.body_mut()).await {
                        // Return different status codes and reasons depending on the error of the
                        // function.
                        let (status_code, reason) = match err.kind() {
                            io::ErrorKind::InvalidData => (StatusCode::BAD_REQUEST, Some("list too long, must be less than 128KB")),
                            _ => (StatusCode::INTERNAL_SERVER_ERROR, None)
                        };
                        utility::default_error_response(status_code, host, reason).await
                    } else {
                        // else, return a new, empty response.
                        // `Bytes::new()` is the same as `Bytes::from_static(b"")`.
                        FatResponse::no_cache(Response::new(Bytes::new()))
                    }
                }
                // In all other cases (more methods exist),
                // send a method not allowed error to the client.
                _ => {
                    utility::default_error_response(StatusCode::METHOD_NOT_ALLOWED, host, None).await
                }
            }
        } else {
            // If a id doesn't exist, return a bad request error, telling the client it should have
            // a id in it's request query.
            utility::default_error_response(StatusCode::BAD_REQUEST, host, Some("You need an ID in the query.")).await
        }
    }));

    // Create a host with hostname "localhost", serving files from directory "./public", and the default extensions.
    // This was changed from "web" to "./", so it serves files from "./public", which is the same
    // as "public" instead of "web/public".
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
    // This will never happen; we don't shut it down anywhere!
    shutdown_manager.wait().await;
}
