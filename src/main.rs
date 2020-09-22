use actix_multipart::Multipart;
use actix_web::client::Client;
use actix_web::{get , web, App, Error, HttpResponse, HttpServer, Responder};

use std::io::Write;
use std::str;

use futures::{StreamExt, TryStreamExt};

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

#[derive(Deserialize, Serialize)]
struct CommitResponse {
    content: CommitResponseContent,
}

#[derive(Deserialize, Serialize)]
struct CommitResponseContent {
    download_url: String,
}

async fn commit(repo: String, access_token: String, content: CommitContent) -> CommitResponse {
    let repository = repo;

    let post_url = format!(
        "https://api.github.com/repos/{}/contents/{}",
        repository, content.path
    );
    let token = access_token;

    let client = Client::default();

    let response: CommitResponse = client
        .put(post_url)
        .header("User-Agent", "actix-web/3.0")
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json;charset=UTF-8")
        .send_body(serde_json::to_string(&content).unwrap())
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    println!("Response: {:?}", response.content.download_url);
    response
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

    let content_string = format!(
        "+++\ntitle = \"{}\"\ndate = {}\n+++\n{}\n\n<!-- more -->",
        title_string, date, body_string
    );

    let c2 = CommitContent::new(
        "Add post".to_string(),
        content_string,
        format!("content/{}.md", filename_date),
    );

    commit(
        content.repo.to_string(),
        content.access_token.to_string(),
        c2,
    )
    .await;
    HttpResponse::Ok().json(content.0)
}

async fn upload_image(mut payload: Multipart) -> Result<web::Json<CommitResponse>> {
    let mut content: Option<CommitContent> = Option::None;
    let mut repo: Option<String> = Option::None;
    let mut access_token: Option<String> = Option::None;

    while let Ok(Some(mut field)) = payload.try_next().await {
        let content_type = field.content_disposition().unwrap();

        if content_type.get_name() == Some("access_token") {
            let mut vec = Vec::new();

            while let Some(chunk) = field.next().await {
                let data = chunk.unwrap();
                let vec_b = data.to_vec();
                vec.extend(vec_b);
            }

            access_token = Some(str::from_utf8(&vec).unwrap().to_string());
            continue;
        }

        if content_type.get_name() == Some("repo") {
            let mut vec = Vec::new();

            while let Some(chunk) = field.next().await {
                let data = chunk.unwrap();
                let vec_b = data.to_vec();
                vec.extend(vec_b);
            }

            repo = Some(str::from_utf8(&vec).unwrap().to_string());

            continue;
        }

        if content_type.get_name() == Some("file") {
            let filename = content_type.get_filename().unwrap();
            let filepath = format!("static/{}", sanitize_filename::sanitize(&filename));

            let mut vec = Vec::new();

            while let Some(chunk) = field.next().await {
                let data = chunk.unwrap();
                let vec_b = data.to_vec();
                vec.extend(vec_b);
            }

            content = Some(CommitContent::new_from_image(
                "Add image".to_string(),
                vec,
                filepath.to_string(),
            ));

            continue;

            // println!("{}", content.message);
        }
    }

    // TODO: Fix unwrapping

    let unwrapped_repo = repo.unwrap();
    let unwrapped_access_token = access_token.unwrap();
    let unwrapped_content = content.unwrap();

    let response = commit(unwrapped_repo, unwrapped_access_token, unwrapped_content).await;

    Ok(web::Json(response))
}

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

async fn manual_hello() -> impl Responder {
    HttpResponse::Ok().body("Hey there!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .service(hello)
            .service(web::resource("/post_content").route(web::post().to(post_content)))
            .service(web::resource("/upload_image").route(web::post().to(upload_image)))
            .route("/hey", web::get().to(manual_hello))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
