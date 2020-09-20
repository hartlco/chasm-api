use actix_web::client::Client;
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};

use json::JsonValue;

use chrono::{DateTime, Utc};

use serde::{Deserialize, Serialize};

use serde_json::{Result, Value};

use base64::encode;

#[derive(Deserialize, Serialize)]
struct CommitContent {
    message: String,
    content: String,
    path: String,
}

impl CommitContent {
    fn new(message: String, content: String, path: String) -> CommitContent {
        CommitContent {
            message: message,
            content: encode(content),
            path: path,
        }
    }

    fn new_from_image(message: String, content: Vec<u8>, path: String) -> CommitContent {
        CommitContent {
            message: message,
            content: encode(content),
            path: path,
        }
    }
}

#[derive(Deserialize, Serialize)]
struct PostContent {
    repo: String,
    access_token: String,
    title: Option<String>,
    content: Vec<ContentPart>,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "type", content = "value")]
enum ContentPart {
    Header(String),
    Paragraph(String),
    Image(String),
}

async fn commit(repo: String, access_token: String, content: CommitContent) {
    let repository = repo;

    let post_url = format!(
        "https://api.github.com/repos/{}/contents/{}",
        repository, content.path
    );
    let token = access_token;

    let client = Client::default();

    let response = client
        .put(post_url)
        .header("User-Agent", "actix-web/3.0")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json;charset=UTF-8")
        .send_body(serde_json::to_string(&content).unwrap())
        .await;

    println!("Response: {:?}", response);
}

async fn post_content(content: web::Json<PostContent>) -> HttpResponse {
    println!("model: {:?}", &content.access_token);

    let now: DateTime<Utc> = Utc::now();
    let filename_date = now.format("%Y-%m-%dT%H:%M:%SZ");
    let date = now.format("%Y-%m-%dT%H:%M:%SZ");

    let mut body_string = "".to_string();

    let mut title_string = "".to_string();

    if let Some(title) = &content.title {
        title_string.push_str(&title);
    }

    for content_part in &content.content {
        match content_part {
            ContentPart::Header(text) => {
                let header_string = format!("## {}\n", text);
                body_string.push_str(&header_string);
            }
            ContentPart::Paragraph(text) => {
                let paragraph_string = format!("{}\n", text);
                body_string.push_str(&paragraph_string);
            }
            ContentPart::Image(link) => {
                let image_string = format!("![]({})\n", link);
                body_string.push_str(&image_string);
            }
        }
        // body_string.push_str(&image_string);
    }

    let content_string = format!("+++\ntitle = \"{}\"\ndate = {}\n+++\n{}\n\n<!-- more -->", title_string, date, body_string);

    let c2 = CommitContent::new(
        "Add post".to_string(),
        content_string,
        format!("content/{}.md", filename_date),
    );

    commit(
        content.repo.to_string(),
        content.access_token.to_string(),
        c2,
    ).await;
    HttpResponse::Ok().json(content.0)
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
