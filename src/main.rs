use http::method::Method;
use httparse::Request;
use log::error;
use std::borrow::Cow;
use std::collections::HashMap;
use std::error::Error;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;

type HandlerFn = fn(&str) -> (String, &'static str, u16);

const HTML_TEMPLATE: &str = r#"
<!DOCTYPE html>
<html>
<head>
    <title>{title}</title>
</head>
<body>
    <h1>{content}</h1>
</body>
</html>
"#;

fn create_response(title: &str, content: &str, status_code: u16) -> (String, &'static str, u16) {
    let response_content = HTML_TEMPLATE
        .replace("{title}", title)
        .replace("{content}", content);

    (response_content, "text/html", status_code)
}

fn handle_hello(_: &str) -> (String, &'static str, u16) {
    create_response("Hello Page", "Hello, Rust HTTP Server!", 200)
}

fn handle_goodbye(_: &str) -> (String, &'static str, u16) {
    create_response("Goodbye Page", "Goodbye, Rust HTTP Server!", 200)
}

fn handle_submit(_: &str) -> (String, &'static str, u16) {
    create_response("Submission Page", "Data submitted successfully!", 200)
}

fn handle_not_found(_: &str) -> (String, &'static str, u16) {
    create_response("404 - Not Found", "Not Found", 404)
}

fn handle_client(
    mut stream: TcpStream,
    routes: Arc<HashMap<(&'static str, Method), HandlerFn>>,
) -> Result<(), Box<dyn Error>> {
    let mut buffer = [0; 1024];
    let read_bytes = stream.read(&mut buffer)?;

    if read_bytes == 0 {
        return Ok(());
    }

    let request = String::from_utf8_lossy(&buffer[..read_bytes]);
    let mut headers = [httparse::EMPTY_HEADER; 16];
    let mut parsed_request = Request::new(&mut headers);

    if let Err(err) = parsed_request.parse(request.as_bytes()) {
        error!("Failed to parse request: {}", err);
        return Ok(());
    }

    let http_method =
        Method::from_bytes(parsed_request.method.unwrap().as_bytes()).expect("Invalid HTTP method");
    let path = Cow::Borrowed(parsed_request.path.unwrap());

    let (response_content, content_type, status_code) = find_handler(&path, http_method, &routes)
        .map_or_else(|| handle_not_found(&request), |handler| handler(&request));

    let response = format!(
        "HTTP/1.1 {}\r\nContent-Length: {}\r\nContent-Type: {}\r\n\r\n{}",
        status_code,
        response_content.len(),
        content_type,
        response_content
    );

    if let Err(err) = stream.write_all(response.as_bytes()) {
        error!("Failed to write response: {}", err);
    }

    if let Err(err) = stream.flush() {
        error!("Failed to flush stream: {}", err);
    }

    Ok(())
}

fn find_handler<'a>(
    path: &str,
    method: Method,
    routes: &HashMap<(&'static str, Method), HandlerFn>,
) -> Option<HandlerFn> {
    routes.get(&(path, method)).copied()
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let listener = TcpListener::bind("127.0.0.1:8080")?;

    let routes: Arc<HashMap<(&str, Method), HandlerFn>> = Arc::new({
        let mut routes: HashMap<(&str, Method), HandlerFn> = HashMap::new();
        routes.insert(("/hello", Method::GET), handle_hello);
        routes.insert(("/bye", Method::GET), handle_goodbye);
        routes.insert(("/submit", Method::POST), handle_submit);
        routes
    });

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let routes = routes.clone();
                std::thread::spawn(move || {
                    if let Err(e) = handle_client(stream, routes) {
                        error!("Error handling client: {}", e);
                    }
                });
            }
            Err(e) => {
                error!("Error accepting connection: {}", e);
            }
        }
    }

    Ok(())
}
