use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};

use json::JsonValue;

use reqwest::header::AUTHORIZATION;
use reqwest::header::CONTENT_TYPE;
use reqwest::header::USER_AGENT;

use serde::{Serialize, Deserialize};

use serde_json::{Result, Value};

#[derive(Deserialize, Serialize)]
struct CommitContent {
    message: String,
    content: String,
    path: String,
}

#[derive(Deserialize, Serialize)]
struct PostContent {
    repo: String,
    access_token: String,
    content: Vec<ContentPart>
}

#[derive(Deserialize, Serialize)]
enum ContentPart {
    Title(String),
    Paragraph(String)
}

fn commit(repo: String, access_token: String, content: CommitContent) {
    let repository = repo;

  let post_url = format!(
        "https://api.github.com/repos/{}/contents/{}", repository, 
        content.path
    );
    let token = access_token;
    let new_put = reqwest::blocking::Client::new()
        .put(&post_url)
        .header(USER_AGENT, "pan-rust")
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .header(CONTENT_TYPE, "application/json;charset=UTF-8")
        .body(serde_json::to_string(&content).unwrap())
        .send()
        .unwrap();
        

    // let commit_response: CommitResponse = new_put.json().unwrap();
    // return commit_response
}

async fn post_content(content: web::Json<PostContent>) -> HttpResponse {
    println!("model: {:?}", &content.access_token);
    HttpResponse::Ok().json(content.0) // <- send response
}

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

#[post("/echo")]
async fn echo(req_body: String) -> impl Responder {
    HttpResponse::Ok().body(req_body)
}

async fn manual_hello() -> impl Responder {
    HttpResponse::Ok().body("Hey there!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .service(hello)
            .service(echo)
            .service(web::resource("/post_content").route(web::post().to(post_content)))
            .route("/hey", web::get().to(manual_hello))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}