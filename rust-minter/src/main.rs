extern crate ethabi;
use ethabi::{Token, Function, Param, ParamType};

use serde_json::Map;
use serde_json::json;
use json::{object, JsonValue};
use std::env;
use hex;

async fn mint_token(
    _client: &hyper::Client<hyper::client::HttpConnector>,
    _server_addr: &str,
    to_address: &str, 
    token_id: u64
) -> Result<(), Box<dyn std::error::Error>>  {
    // Define the mint function according to the ABI of your ERC-721 contract
    let mint_function = Function {
        name: "safeMint".to_owned(),
        inputs: vec![
            Param { name: "to".to_owned(), kind: ParamType::Address, internal_type: None },
            Param { name: "tokenId".to_owned(), kind: ParamType::Uint(256), internal_type: None },
        ],
        outputs: vec![],
        state_mutability: ethabi::StateMutability::NonPayable,
        constant: false
    };

    // Encode the inputs for the mint function
    let mint_payload = mint_function.encode_input(&[
        Token::Address(to_address.parse().expect("Invalid to address")),
        Token::Uint(token_id.into()),
    ]).expect("Encoding failed");

    let hex: String = mint_payload.iter().map(|byte| format!("{:02x}", byte)).collect();
    let ethereum_hex = format!("0x{}", hex);

    let response = object! {
        "destination" => format!("{}", "0xc6e7DF5E7b4f2A278906862b61205850344D4e7d"),
        "payload" => format!("{}", ethereum_hex)
    };

    let request = hyper::Request::builder()
        .method(hyper::Method::POST)
        .header(hyper::header::CONTENT_TYPE, "application/json")
        .uri(format!("{}/voucher", &_server_addr))
        .body(hyper::Body::from(response.dump()))?;
    let response = _client.request(request).await?;

    println!("SUCCESS");

    Ok(())
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

    if operation_str.as_str().starts_with("mint") {
        mint_token(_client, _server_addr, "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266", 6).await?;
    }    

    let mut data = Map::new();
    data.insert("operation".to_string(), json!(operation_str));
    let serialized = serde_json::to_string(&data).unwrap();
    let hex: String = serialized.as_bytes().iter().map(|b| format!("{:02x}", b)).collect();
    let ethereum_hex = format!("0x{}", hex);

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
