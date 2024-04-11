#[macro_use]
extern crate lazy_static;
use std::sync::Mutex;
use serde_json::Map;
use serde_json::json;
use json::{object, JsonValue};
use std::env;
use hex;

lazy_static! {
    static ref COUNTER: Mutex<i32> = Mutex::new(0);
}

pub async fn handle_advance(
    _client: &hyper::Client<hyper::client::HttpConnector>,
    _server_addr: &str,
    request: JsonValue,
) -> Result<&'static str, Box<dyn std::error::Error>> {
    println!("Received advance request data {}", &request);
    let _payload = request["data"]["payload"]
        .as_str()
        .ok_or("Missing payload")?;
    // TODO: add application logic here

    let bytes = hex::decode(&_payload[2..]).expect("Decoding failed");
    let operation_str = String::from_utf8(bytes).expect("Invalid UTF-8 sequence");

    println!("{:?}", operation_str);

    let mut num = COUNTER.lock().unwrap();
    match operation_str.as_str() {
        "add" | "+" => *num += 1,
        "subtract" | "-" => *num -= 1,
        _ => println!("Unsupported operation"),
    }
    println!("Counter value: {}", *num);
    

    let mut data = Map::new();
    data.insert("operation".to_string(), json!(operation_str));
    data.insert("value".to_string(), json!(*num));
    let serialized = serde_json::to_string(&data).unwrap();
    let hex: String = serialized.as_bytes().iter().map(|b| format!("{:02x}", b)).collect();
    let ethereum_hex = format!("0x{}", hex);

    println!("Counter hex value: {}", ethereum_hex);

    let response = object! {
        "payload" => format!("{}", ethereum_hex)
    };

    let request = hyper::Request::builder()
        .method(hyper::Method::POST)
        .header(hyper::header::CONTENT_TYPE, "application/json")
        .uri(format!("{}/notice", &_server_addr))
        .body(hyper::Body::from(response.dump()))?;
    let response = _client.request(request).await?;
    
    Ok("accept")
}

pub async fn handle_inspect(
    _client: &hyper::Client<hyper::client::HttpConnector>,
    _server_addr: &str,
    request: JsonValue,
) -> Result<&'static str, Box<dyn std::error::Error>> {
    println!("Received inspect request data {}", &request);
    let _payload = request["data"]["payload"]
        .as_str()
        .ok_or("Missing payload")?;
    // TODO: add application logic here

    let bytes = hex::decode(&_payload[2..]).expect("Decoding failed");
    let params = String::from_utf8(bytes).expect("Invalid UTF-8 sequence");
    println!("{:?}", params);
    if params.to_string().starts_with("counter") {
        let mut num = COUNTER.lock().unwrap();
        let mut data = Map::new();
        data.insert("value".to_string(), json!(*num));
        let serialized = serde_json::to_string(&data).unwrap();
        let hex: String = serialized.as_bytes().iter().map(|b| format!("{:02x}", b)).collect();
        let ethereum_hex = format!("0x{}", hex);
    
        let response = object! {
            "payload" => format!("{}", ethereum_hex)
        };
    
        let request = hyper::Request::builder()
            .method(hyper::Method::POST)
            .header(hyper::header::CONTENT_TYPE, "application/json")
            .uri(format!("{}/report", &_server_addr))
            .body(hyper::Body::from(response.dump()))?;
        let response = _client.request(request).await?;
    }


    Ok("accept")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = hyper::Client::new();
    let server_addr = env::var("ROLLUP_HTTP_SERVER_URL")?;

    let mut status = "accept";
    loop {
        println!("Sending finish");
        let response = object! {"status" => status.clone()};
        let request = hyper::Request::builder()
            .method(hyper::Method::POST)
            .header(hyper::header::CONTENT_TYPE, "application/json")
            .uri(format!("{}/finish", &server_addr))
            .body(hyper::Body::from(response.dump()))?;
        let response = client.request(request).await?;
        println!("Received finish status {}", response.status());

        if response.status() == hyper::StatusCode::ACCEPTED {
            println!("No pending rollup request, trying again");
        } else {
            let body = hyper::body::to_bytes(response).await?;
            let utf = std::str::from_utf8(&body)?;
            let req = json::parse(utf)?;

            let request_type = req["request_type"]
                .as_str()
                .ok_or("request_type is not a string")?;
            status = match request_type {
                "advance_state" => handle_advance(&client, &server_addr[..], req).await?,
                "inspect_state" => handle_inspect(&client, &server_addr[..], req).await?,
                &_ => {
                    eprintln!("Unknown request type");
                    "reject"
                }
            };
        }
    }
}
